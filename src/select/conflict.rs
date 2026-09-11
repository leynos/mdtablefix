//! The two predicates that make a mid-merge rewrite safe.
//!
//! Depends on `std::fs` and `camino`. Both predicates are deliberately narrow:
//! [`has_conflict_markers`] requires all three marker forms, each at the start
//! of a line with an exact seven-character run, so a document *discussing*
//! conflict markers is not mistaken for a conflicted one; and
//! [`operation_in_progress`] narrows the scan further, to a repository actually
//! mid-merge, mid-rebase, or mid-cherry-pick.
//!
//! Reflowing across the markers restructures text on both sides of the
//! boundary, so the user would resolve against corrupted content and commit it
//! into a rewritten history, where `git rebase --abort` is gone.

use camino::Utf8Path;

/// The entries a Git directory holds only while an operation is paused, each
/// named as `git rev-parse --git-path` would report it.
///
/// `ORIG_HEAD`, `SQUASH_MSG`, and `COMMIT_EDITMSG` also appear under a Git
/// directory and are deliberately absent: an idle repository holds them, so
/// they say nothing about an operation in progress.
const IN_PROGRESS: [&str; 4] = [
    "MERGE_HEAD",
    "rebase-merge",
    "rebase-apply",
    "CHERRY_PICK_HEAD",
];

/// The three marker forms a conflicted hunk carries, in the order Git writes
/// them: the first side, the separator, and the second side.
const MARKERS: [char; 3] = ['<', '=', '>'];

/// The length of the run that begins a marker line.
///
/// Git writes exactly seven. A longer run is some other construct — eight `<`
/// is Markdown emphasis, and a setext heading underline is seven `=` at the
/// start of a line — so the run is checked for exactness rather than merely
/// counted up to.
const MARKER_LEN: usize = 7;

/// Reports whether the repository is mid-merge, mid-rebase, or
/// mid-cherry-pick, by testing for `MERGE_HEAD`, `rebase-merge`,
/// `rebase-apply`, and `CHERRY_PICK_HEAD` under the Git directory.
///
/// A presence test rather than a Git subprocess: the Git directory is already
/// resolved by the caller, and this runs once per `--git` invocation, not
/// once per candidate.
pub fn operation_in_progress(git_dir: &Utf8Path) -> bool {
    IN_PROGRESS
        .iter()
        .any(|name| std::fs::symlink_metadata(git_dir.join(name)).is_ok())
}

/// Reports whether `content` carries all three conflict-marker forms, each at
/// the start of a line with an exact seven-character run.
///
/// All three are required because any one of them alone is ordinary Markdown:
/// `=======` underlines a setext heading, and prose or a fenced example may
/// name the other two. This is not a Markdown parser, so a fenced example
/// carrying all three is indistinguishable from a conflict — one reason the
/// scan is gated on [`operation_in_progress`], and the reason
/// `--allow-conflicted` exists.
#[must_use]
pub fn has_conflict_markers(content: &str) -> bool {
    MARKERS.iter().all(|marker| {
        content
            .lines()
            .any(|line| starts_with_marker(line, *marker))
    })
}

/// Whether `line` begins with exactly [`MARKER_LEN`] copies of `marker`.
fn starts_with_marker(line: &str, marker: char) -> bool {
    let mut characters = line.chars();

    (0..MARKER_LEN).all(|_| characters.next() == Some(marker)) && characters.next() != Some(marker)
}

#[cfg(test)]
#[path = "conflict_tests.rs"]
mod tests;
