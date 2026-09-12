//! The two predicates that make a mid-merge rewrite safe.
//!
//! Depends on `cap_std::fs_utf8`, `std::io`, and `camino`. Both predicates are
//! deliberately narrow: [`has_conflict_markers`] requires all three marker
//! forms, each at the start of a line with a run of at least seven characters,
//! so a document *discussing* conflict markers is not mistaken for a conflicted
//! one; and [`operation_in_progress`] narrows the question further, to a
//! repository actually mid-merge, mid-rebase, mid-revert, or mid-cherry-pick.
//!
//! The repository is asked at the write boundary rather than once per run: a
//! merge, rebase, or revert can begin while a long run is still analysing
//! files, and a run-wide snapshot would let exactly that run rewrite the
//! conflict it started inside. [`ConflictGuard::refuses`] is therefore the one
//! caller of [`operation_in_progress`], and it asks only about a file whose
//! markers have already been found.
//!
//! Reflowing across the markers restructures text on both sides of the
//! boundary, so the user would resolve against corrupted content and commit it
//! into a rewritten history, where `git rebase --abort` is gone.

use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

/// The entries a Git directory holds only while an operation is paused, each
/// named as `git rev-parse --git-path` would report it.
///
/// `REVERT_HEAD` is the pseudoref `git revert` writes when it stops on a
/// conflict, and it is the reason a revert is named beside the other three in
/// this module's own prose. `ORIG_HEAD`, `SQUASH_MSG`, and `COMMIT_EDITMSG`
/// also appear under a Git directory and are deliberately absent: an idle
/// repository holds them, so they say nothing about an operation in progress.
const IN_PROGRESS: [&str; 5] = [
    "MERGE_HEAD",
    "rebase-merge",
    "rebase-apply",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
];

/// The three marker forms a conflicted hunk carries, in the order Git writes
/// them: the first side, the separator, and the second side.
const MARKERS: [char; 3] = ['<', '=', '>'];

/// The shortest run that begins a marker line.
///
/// Seven is Git's default, and not its only length: a repository that sets
/// `conflict-marker-size` in `.gitattributes` gets runs of exactly the
/// configured length, so the run is required to reach seven rather than to end
/// there. Querying the attribute instead would mean resolving it per path, and
/// guessing low is the failure that matters — a run of eight read as ordinary
/// Markdown is a resolution rewritten by this tool — while guessing high costs
/// only a refusal `--allow-conflicted` overrides.
const MARKER_LEN: usize = 7;

/// Whether a rewrite must refuse a file that carries conflict markers.
///
/// The guard holds where the repository is, not what it said: the answer is
/// read at the moment a file is about to be replaced, so a run that spans the
/// start of a merge cannot act on the state the run began under. See
/// [`Self::refuses`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictGuard {
    /// Nothing to consult.
    ///
    /// A run that cannot write — `--print`, `--check`, `--diff`, `--list-files`
    /// — and a run whose user passed `--allow-conflicted`. The first has no
    /// rewrite to refuse; the second has asked for the rewrite whatever the
    /// repository is doing. Neither pays for a second `git` process, which is
    /// also what keeps an ordinary run from spawning `git` at all.
    Unguarded,
    /// The Git directory to consult immediately before each rewrite.
    Guarded(Utf8PathBuf),
}

impl ConflictGuard {
    /// The guard for a run with no repository to consult.
    ///
    /// A path the user named on the command line is not a selection this tool
    /// made, so it is not this tool's to refuse, and no repository is consulted
    /// for one.
    #[must_use]
    pub const fn unguarded() -> Self { Self::Unguarded }

    /// The guard for a run that must ask this Git directory before it writes.
    #[must_use]
    pub fn guarded(git_dir: impl Into<Utf8PathBuf>) -> Self { Self::Guarded(git_dir.into()) }

    /// Whether `content` must not be rewritten, asking the repository now.
    ///
    /// The marker scan comes first because it is in memory and the repository
    /// is not: a file that does not carry all three markers cannot be refused
    /// whatever the repository is doing, so the filesystem is read for a file a
    /// refusal could be about and for no other. This is also what keeps the
    /// false positive contained — a document that quotes all three markers
    /// inside a fenced block is indistinguishable from a conflicted one, and
    /// refusing to rewrite it would be a false alarm about a file nothing is
    /// merging.
    ///
    /// # Errors
    ///
    /// Returns an error if `content` carries conflict markers and the state of
    /// the Git directory cannot be read. A rewrite whose safety cannot be
    /// established is not a rewrite this tool performs: the caller reports the
    /// file as an error rather than writing it.
    pub fn refuses(&self, content: &str) -> Result<bool, RepositoryStateError> {
        let Self::Guarded(git_dir) = self else {
            return Ok(false);
        };
        if !has_conflict_markers(content) {
            return Ok(false);
        }

        operation_in_progress(git_dir)
    }
}

/// Reports whether the repository is mid-merge, mid-rebase, mid-revert, or
/// mid-cherry-pick, by testing for `MERGE_HEAD`, `rebase-merge`,
/// `rebase-apply`, `CHERRY_PICK_HEAD`, and `REVERT_HEAD` under the Git
/// directory.
///
/// A presence test rather than a Git subprocess: the Git directory is already
/// resolved by the caller, and this runs at the write boundary — once for each
/// file that carries conflict markers — rather than once per candidate.
///
/// The directory is opened as a capability and the markers are read as fixed
/// relative entries of it. The names are this module's own constants, so no
/// path a repository or a user wrote takes part in the resolution, and the
/// capability is what makes a stored path unnecessary. It is the one place in
/// the selection that opens a directory for itself; see ADR 0010 for why the
/// probe, whose subject *is* the ambient tree, does not.
///
/// # Errors
///
/// Returns an error if the Git directory cannot be opened, or if a marker
/// cannot be tested for other than by being absent. Absence is the ordinary
/// answer and means only that this marker is not there; any other failure —
/// including a Git directory that has gone since it was resolved, which
/// `open_ambient_dir` reports rather than passing on as an idle repository —
/// means the question went unanswered, and an unanswered question is not a
/// licence to write.
pub fn operation_in_progress(git_dir: &Utf8Path) -> Result<bool, RepositoryStateError> {
    let directory = Dir::open_ambient_dir(git_dir, ambient_authority()).map_err(|source| {
        RepositoryStateError {
            git_dir: git_dir.to_owned(),
            source,
        }
    })?;

    for name in IN_PROGRESS {
        let present =
            marker_present(directory.symlink_metadata(name).map(|_| ())).map_err(|source| {
                RepositoryStateError {
                    git_dir: git_dir.to_owned(),
                    source,
                }
            })?;
        if present {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Whether the marker just tested for is present, from the result of testing.
///
/// `Ok(true)` is a marker that is there, `Ok(false)` one that is not, and an
/// error is a question that went unanswered rather than a marker that is
/// absent. It is a function of the result rather than a match arm of the scan
/// above, for the reason the probe's `unnameable` is one: the capability
/// refuses a Git directory it cannot open before the loop begins, so no fixture
/// reaches that arm through the filesystem — and reading an unreadable marker
/// as an absent one is the mistake that would license a rewrite during a merge.
fn marker_present(test: io::Result<()>) -> Result<bool, io::Error> {
    match test {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// The state of a Git directory could not be read.
///
/// Carries the directory, because a marker that cannot be tested for is only
/// actionable with its location: the caller prints one line, and a bare
/// `Permission denied` would name neither what failed nor where.
#[derive(Debug, thiserror::Error)]
#[error("reading the state of the Git directory `{git_dir}`")]
pub struct RepositoryStateError {
    /// The Git directory whose markers were being tested for.
    pub git_dir: Utf8PathBuf,
    /// Why the directory could not be opened, or an entry of it tested for.
    #[source]
    pub source: io::Error,
}

/// Reports whether `content` carries all three conflict-marker forms, each at
/// the start of a line with a run of at least seven characters.
///
/// All three are required because any one of them alone is ordinary Markdown:
/// `=======` underlines a setext heading, and prose or a fenced example may
/// name the other two. This is not a Markdown parser, so a fenced example
/// carrying all three is indistinguishable from a conflict — one reason
/// [`ConflictGuard::refuses`] asks the repository and the content together
/// rather than the content alone, and the reason `--allow-conflicted` exists.
#[must_use]
pub fn has_conflict_markers(content: &str) -> bool {
    MARKERS.iter().all(|marker| {
        content
            .lines()
            .any(|line| starts_with_marker(line, *marker))
    })
}

/// Whether `line` begins with a run of at least [`MARKER_LEN`] copies of
/// `marker`.
fn starts_with_marker(line: &str, marker: char) -> bool {
    let mut characters = line.chars();

    (0..MARKER_LEN).all(|_| characters.next() == Some(marker))
}

#[cfg(test)]
#[path = "conflict_tests.rs"]
mod tests;
