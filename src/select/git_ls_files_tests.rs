//! Tests for the `git ls-files` adapter.
//!
//! The boundary test runs the real `git`, because the framing it depends on —
//! NUL-terminated, unquoted, verbatim paths relative to the process working
//! directory — is exactly the part a fake would assume. It inherits the ambient
//! Git configuration, as the adapter does; the fixture pins the branch name and
//! commits nothing, so the settings that could perturb it are not in play, and
//! the assertion is on the whole listing rather than on a subset, so a
//! perturbation would be loud.

use std::cell::Cell;

use camino::{Utf8Path, Utf8PathBuf};
use proptest::{collection::vec, prelude::*, test_runner::TestRunner};
use rstest::rstest;

use super::{
    CandidateListing,
    GitListError,
    GitLsFiles,
    RELAYED_LIMIT,
    relayable,
    split_nul_delimited,
};

/// Printable ASCII, the common case for a repository path.
fn ascii_text() -> impl Strategy<Value = Vec<u8>> { vec(0x20u8..=0x7e, 1..=40) }

/// Arbitrary bytes that are not NUL and so can be a path in Git's framing.
fn raw_bytes() -> impl Strategy<Value = Vec<u8>> {
    vec(
        any::<u8>().prop_filter("a path holds no NUL byte", |byte| *byte != 0),
        1..=40,
    )
}

/// A path-shaped byte string that is sometimes not valid UTF-8.
fn segment() -> impl Strategy<Value = Vec<u8>> { prop_oneof![3 => ascii_text(), 1 => raw_bytes()] }

/// INV-NUL-SPLIT: splitting is a faithful inverse of Git's framing.
#[test]
fn splitting_is_a_faithful_inverse_of_nul_framing() {
    let mut runner = TestRunner::default();
    let saw_non_utf8 = Cell::new(false);
    let saw_empty_input = Cell::new(false);

    runner
        .run(&vec(segment(), 0..=20), |segments| {
            let mut framed = Vec::new();
            for segment in &segments {
                framed.extend_from_slice(segment);
                framed.push(0);
            }

            let listing = split_nul_delimited(&framed);
            let textual: Vec<&str> = segments
                .iter()
                .filter_map(|segment| std::str::from_utf8(segment).ok())
                .collect();

            prop_assert_eq!(listing.skipped_non_utf8, segments.len() - textual.len());
            prop_assert_eq!(
                listing
                    .paths
                    .iter()
                    .map(|path| path.as_str())
                    .collect::<Vec<_>>(),
                textual
            );

            if listing.skipped_non_utf8 > 0 {
                saw_non_utf8.set(true);
            }
            if segments.is_empty() {
                saw_empty_input.set(true);
                prop_assert!(
                    listing.paths.is_empty(),
                    "empty input must yield an empty list, not a list holding one empty path"
                );
            }
            Ok(())
        })
        .expect("splitting is a faithful inverse of NUL framing");

    assert!(
        saw_non_utf8.get(),
        "the generator must reach the drop-and-count path, or that path is untested"
    );
    assert!(
        saw_empty_input.get(),
        "the empty-input case must be generated and asserted, not assumed"
    );
}

#[rstest]
#[case(b"", &[], 0)]
#[case(b"\0", &[], 0)]
#[case(b"a.md\0", &["a.md"], 0)]
#[case(b"a.md\0b.md\0", &["a.md", "b.md"], 0)]
// Git always terminates the last path, but a truncated stream must not invent
// one either way: the final entry is returned, and no empty path is.
#[case(b"a.md", &["a.md"], 0)]
#[case(b"a.md\0\xff\xfe.md\0b.md\0", &["a.md", "b.md"], 1)]
fn splitting_cases(#[case] input: &[u8], #[case] expected: &[&str], #[case] skipped: usize) {
    let listing = split_nul_delimited(input);
    assert_eq!(
        listing.paths,
        expected
            .iter()
            .copied()
            .map(Utf8PathBuf::from)
            .collect::<Vec<_>>()
    );
    assert_eq!(listing.skipped_non_utf8, skipped);
}

/// The listing's paths as sorted text, so the assertion is about membership
/// rather than about Git's output order, which INV-ORDER-DET exists because it
/// is not sorted.
fn names(listing: &CandidateListing) -> Vec<String> {
    let mut names: Vec<String> = listing
        .paths
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    names.sort();
    names
}

fn write(root: &Utf8Path, name: &str, content: &str) {
    let path = root.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the fixture directory");
    }
    std::fs::write(&path, content).expect("write a fixture");
}

/// Runs `git` in `directory`, requiring it to succeed.
///
/// The environment is hardened rather than inherited, as the integration
/// fixtures harden theirs. `GIT_CONFIG_GLOBAL=/dev/null` removes the
/// developer's configuration along with their identity, so the fixture supplies
/// the identity through the environment, and neither a machine-wide commit hook
/// nor a signing key can change what the fixture commits. A runner with no
/// configured identity would otherwise fail at the first `git commit`.
fn git(directory: &Utf8Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(directory)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("HOME", directory)
        .env("LC_ALL", "C")
        .env("LANGUAGE", "")
        .env("GIT_AUTHOR_NAME", "mdtablefix tests")
        .env("GIT_AUTHOR_EMAIL", "tests@example.invalid")
        .env("GIT_COMMITTER_NAME", "mdtablefix tests")
        .env("GIT_COMMITTER_EMAIL", "tests@example.invalid")
        .output()
        .expect("git on PATH; the boundary test needs a real one");
    assert!(
        output.status.success(),
        "git {args:?} failed with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// AX-GIT-LSFILES at the boundary: the framing, the index-only default, and the
/// untracked extension, against the `git` on this machine.
///
/// The ignored file is the assertion with teeth: it is present, it is named
/// like a Markdown file, and it must not be listed.
#[test]
fn lists_the_index_and_the_untracked_files_on_request() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = Utf8Path::from_path(directory.path()).expect("a UTF-8 temporary directory");
    git(root, &["init", "--quiet", "-b", "main"]);
    write(root, ".gitignore", "build/\n");
    write(root, "docs/guide.md", "|A|B|\n");
    write(root, "src/lib.rs", "fn main() {}\n");
    write(root, "notes.md", "|C|D|\n");
    write(root, "build/out.md", "|E|F|\n");
    git(
        root,
        &["add", "--", ".gitignore", "docs/guide.md", "src/lib.rs"],
    );

    let tracked = GitLsFiles::new(false)
        .list_candidates(root)
        .expect("git ls-files");
    assert_eq!(
        names(&tracked),
        [".gitignore", "docs/guide.md", "src/lib.rs"],
        "the default listing is the index alone"
    );
    assert_eq!(tracked.skipped_non_utf8, 0);

    let untracked = GitLsFiles::new(true)
        .list_candidates(root)
        .expect("git ls-files");
    assert_eq!(
        names(&untracked),
        [".gitignore", "docs/guide.md", "notes.md", "src/lib.rs"],
        "the extension adds what is neither ignored nor in the index"
    );
}

#[test]
fn an_absent_program_is_reported_as_such_rather_than_as_a_git_failure() {
    let error = GitLsFiles::with_program("mdtablefix-no-such-program-6f2c", false)
        .list_candidates(Utf8Path::new("."))
        .expect_err("no such program exists");
    assert!(
        matches!(error, GitListError::ProgramNotFound { .. }),
        "{error}"
    );
    assert_eq!(
        error.to_string(),
        "`mdtablefix-no-such-program-6f2c` is not installed or not on PATH"
    );
}

#[cfg(unix)]
#[test]
fn a_failing_command_is_reported_with_its_status_and_gits_own_stderr() {
    let error = GitLsFiles::with_program("/bin/false", false)
        .list_candidates(Utf8Path::new("."))
        .expect_err("`false` always fails");
    let GitListError::Failed {
        command,
        status,
        stderr,
    } = error
    else {
        panic!("expected a git failure, got {error}");
    };
    assert_eq!(command, "/bin/false ls-files");
    assert_eq!(status.code(), Some(1));
    assert!(stderr.is_empty(), "`false` writes nothing: {stderr:?}");
}

/// The command is named as the caller spelled its program, so a test that
/// drives the failure paths with something other than `git` reads as that
/// program's failure rather than as a misleading reference to `git`.
#[cfg(unix)]
#[test]
fn a_failure_names_the_command_with_the_program_that_ran() {
    let error = GitLsFiles::with_program("/bin/false", false)
        .list_candidates(Utf8Path::new("."))
        .expect_err("`false` always fails");

    assert_eq!(
        error.to_string(),
        "`/bin/false ls-files` failed with exit status: 1"
    );
}

/// A failure with nothing of Git's to relay is this tool's own wording alone.
///
/// The guard on the empty `stderr` is load bearing rather than cosmetic: drop
/// it and the line ends with the separator and nothing after it, which reads as
/// a diagnostic that was truncated rather than as a failure that had none.
#[cfg(unix)]
#[test]
fn a_failure_with_no_git_text_carries_our_wording_alone() {
    let error = GitLsFiles::with_program("/bin/false", false)
        .list_candidates(Utf8Path::new("."))
        .expect_err("`false` always fails");

    let diagnostic = error.diagnostic();

    assert_eq!(diagnostic, error.to_string());
    assert!(
        !diagnostic.ends_with(": "),
        "a dangling separator reads as a truncated diagnostic: {diagnostic:?}"
    );
}

/// The Git directory is asked of Git, not guessed from a `.git` entry.
///
/// Run against a linked worktree, which is the case a directory walk gets
/// wrong: its `.git` is a file naming a directory under the main repository's
/// administrative area, so the guard would look for `MERGE_HEAD` in a file.
#[test]
fn the_git_directory_is_resolved_through_git() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = Utf8Path::from_path(directory.path()).expect("a UTF-8 temporary directory");
    git(root, &["init", "--quiet", "-b", "main"]);
    write(root, "docs/guide.md", "|A|B|\n");
    git(root, &["add", "--", "docs/guide.md"]);
    git(root, &["commit", "-m", "initialise"]);
    let worktree = root.join("linked");
    git(
        root,
        &[
            "worktree",
            "add",
            "--quiet",
            worktree.as_str(),
            "-b",
            "side",
        ],
    );

    let resolved = GitLsFiles::new(false)
        .resolve_git_dir(&worktree)
        .expect("git rev-parse --absolute-git-dir");

    // Compared by path components rather than as text, so the assertion holds
    // where the platform spells the separator the other way round.
    assert!(
        resolved
            .as_path()
            .ends_with(Utf8Path::new("worktrees").join("linked")),
        "a linked worktree's Git directory is not the main repository's: {resolved}"
    );
    assert!(
        std::fs::symlink_metadata(resolved.join("HEAD")).is_ok(),
        "{resolved} must be the directory git itself reports"
    );
}

/// A directory outside any repository is a failure rather than an empty
/// answer, and the failure carries git's own diagnostic as one line.
#[test]
fn a_directory_outside_a_repository_has_no_git_directory() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let outside = Utf8Path::from_path(directory.path()).expect("a UTF-8 temporary directory");

    let error = GitLsFiles::new(false)
        .resolve_git_dir(outside)
        .expect_err("no repository governs a fresh temporary directory");

    let message = error.to_string();
    // `ExitStatus`'s own rendering is "exit status: 128" on Unix and
    // "exit code: 128" on Windows, so the assertion pins the part this crate
    // words and leaves the status to the standard library.
    assert!(
        message.starts_with("`git rev-parse` failed with exit "),
        "unexpected message: {message}"
    );
    let diagnostic = error.diagnostic();
    assert!(
        diagnostic.starts_with(&message) && diagnostic.len() > message.len(),
        "the diagnostic must relay git's own text beside our own: {diagnostic:?}"
    );
    assert!(
        !diagnostic.contains('\n') && !diagnostic.contains('\r'),
        "the relayed diagnostic must be one line: {diagnostic:?}"
    );
}

#[test]
fn the_repository_query_reports_an_absent_program_as_such() {
    let error = GitLsFiles::with_program("mdtablefix-no-such-program-6f2c", false)
        .resolve_git_dir(Utf8Path::new("."))
        .expect_err("no such program exists");

    assert!(
        matches!(error, GitListError::ProgramNotFound { .. }),
        "{error}"
    );
}

/// `relayable`: Git's text is scrubbed into one line before it is shown.
#[rstest]
#[case(b"", "")]
#[case(b"fatal: bad revision\n", "fatal: bad revision")]
// A message that is several lines is shown as one, so a repository cannot
// forge additional lines of this tool's stderr.
#[case(b"fatal: bad\nusage: git ls-files\n", "fatal: bad usage: git ls-files")]
#[case(b"a\r\nb\rc", "a b c")]
// A run at either end disappears rather than becoming a gap.
#[case(b"\n\nfatal\n\n", "fatal")]
// An escape sequence loses its escape character, so the rest is inert text
// rather than a terminal instruction a path in the repository chose.
#[case(b"\x1b[31mfatal\x1b[0m", "[31mfatal [0m")]
#[case(b"a\x07b", "a b")]
// Bytes that are not UTF-8 become the replacement character: this is a
// diagnostic, not a path, and nothing acts on it.
#[case(b"bad \xff byte", "bad \u{fffd} byte")]
fn relayed_diagnostics_are_scrubbed_into_one_line(#[case] input: &[u8], #[case] expected: &str) {
    assert_eq!(relayable(input), expected);
}

/// A diagnostic long enough to bury the message it supports is cut, and the
/// cut is visible rather than silent.
///
/// The cap falls at the limit rather than one side of it: a run of exactly
/// [`RELAYED_LIMIT`] characters is relayed whole, so the shortening of a
/// longer one is never mistaken for git having said that much.
#[test]
fn a_diagnostic_of_exactly_the_limit_is_relayed_whole() {
    let at_limit = "x".repeat(RELAYED_LIMIT);

    assert_eq!(relayable(at_limit.as_bytes()), at_limit);
}

/// The character past the limit is what costs the last one its place, and the
/// ellipsis is what says so.
#[test]
fn a_diagnostic_past_the_limit_is_cut() {
    let flood = "x".repeat(4096);

    let relayed = relayable(flood.as_bytes());

    assert_eq!(relayed.chars().count(), RELAYED_LIMIT + 1, "{relayed:?}");
    assert!(relayed.ends_with('…'), "{relayed:?}");
    assert!(
        relayed.starts_with(&"x".repeat(RELAYED_LIMIT)),
        "the cap must keep a prefix, not drop the message: {relayed:?}"
    );
}
