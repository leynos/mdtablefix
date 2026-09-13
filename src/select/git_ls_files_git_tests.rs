//! Boundary tests for the `git` invocations themselves.
//!
//! These run a real program, because what they pin is exactly the part a fake
//! would assume: NUL-terminated, unquoted, verbatim paths relative to the
//! process working directory, and the failure Git reports when it is handed a
//! directory that no repository governs. The fixture drives `git` through
//! [`git`], which hardens its own environment rather than inheriting the
//! developer's: system and global Git configuration are disabled, and the
//! private home directory, locale, and commit identity are the fixture's own.
//! The adapter's env discipline is the subject of the sibling
//! `git_ls_files_tests`, so the fixture is free to be stricter than it. It
//! pins the branch name, and the one test that needs history commits under that
//! supplied identity, so the settings that could perturb it are not in play,
//! and the listing assertions are on the whole listing rather than on a
//! subset, so a perturbation would be loud.
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::{GitListError, GitLsFiles};
use crate::select::git_output::CandidateListing;

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
    let directory =
        Dir::open_ambient_dir(root, ambient_authority()).expect("open fixture directory");
    let name = Utf8Path::new(name);
    if let Some(parent) = name.parent().filter(|parent| !parent.as_str().is_empty()) {
        directory
            .create_dir_all(parent)
            .expect("create the fixture directory");
    }
    directory.write(name, content).expect("write a fixture");
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
    let mut command = std::process::Command::new("git");
    command
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
        .env("GIT_COMMITTER_EMAIL", "tests@example.invalid");
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
    ] {
        command.env_remove(variable);
    }
    let output = command
        .output()
        .expect("git on PATH; the boundary test needs a real one");
    assert!(
        output.status.success(),
        "git {args:?} failed with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Initialises a repository in a fresh temporary directory.
fn repository() -> (tempfile::TempDir, Utf8PathBuf) {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let root = Utf8Path::from_path(directory.path())
        .expect("a UTF-8 temporary directory")
        .to_owned();
    git(&root, &["init", "--quiet", "-b", "main"]);

    (directory, root)
}

/// AX-GIT-LSFILES at the boundary: the framing, the index-only default, and the
/// untracked extension, against the `git` on this machine.
///
/// The ignored file is the assertion with teeth: it is present, it is named
/// like a Markdown file, and it must not be listed.
#[test]
fn lists_the_index_and_the_untracked_files_on_request() {
    let (_directory, root) = repository();
    write(&root, ".gitignore", "build/\n");
    write(&root, "docs/guide.md", "|A|B|\n");
    write(&root, "src/lib.rs", "fn main() {}\n");
    write(&root, "notes.md", "|C|D|\n");
    write(&root, "build/out.md", "|E|F|\n");
    git(
        &root,
        &["add", "--", ".gitignore", "docs/guide.md", "src/lib.rs"],
    );

    let tracked = GitLsFiles::new(false)
        .list_candidates(&root)
        .expect("git ls-files");
    assert_eq!(
        names(&tracked),
        [".gitignore", "docs/guide.md", "src/lib.rs"],
        "the default listing is the index alone"
    );
    assert_eq!(tracked.skipped_non_utf8, 0);

    let untracked = GitLsFiles::new(true)
        .list_candidates(&root)
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
    let (_directory, root) = repository();
    write(&root, "docs/guide.md", "|A|B|\n");
    git(&root, &["add", "--", "docs/guide.md"]);
    git(&root, &["commit", "-m", "initialise"]);
    let worktree = root.join("linked");
    git(
        &root,
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
