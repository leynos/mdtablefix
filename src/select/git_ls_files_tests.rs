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

use super::{CandidateListing, GitListError, GitLsFiles, split_nul_delimited};

/// A printable ASCII string, the common case for a repository path.
fn ascii_text() -> impl Strategy<Value = Vec<u8>> {
    vec(0x20u8..=0x7e, 1..=40).prop_map(String::into_bytes)
}

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

            prop_assert_eq!(
                listing
                    .paths
                    .iter()
                    .map(Utf8PathBuf::as_str)
                    .collect::<Vec<_>>(),
                textual
            );
            prop_assert_eq!(listing.skipped_non_utf8, segments.len() - textual.len());

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
    let mut names: Vec<String> = listing.paths.iter().map(|path| path.to_string()).collect();
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
fn git(directory: &Utf8Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(directory)
        .args(args)
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
    assert!(
        error.to_string().contains("git ls-files"),
        "the failure must name the command the user could run: {error}"
    );
    let GitListError::Failed { status, stderr } = error else {
        panic!("expected a git failure, got {error}");
    };
    assert_eq!(status.code(), Some(1));
    assert!(stderr.is_empty(), "`false` writes nothing: {stderr:?}");
}
