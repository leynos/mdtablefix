//! Step definitions for `tests/features/git_file_selection.feature`.
//!
//! Every step drives the real binary through `assert_cmd` and builds a real
//! repository with real `git`, so the feature specifies the command-line
//! contract rather than a reimplementation of it. The bindings themselves are
//! in `tests/git_file_selection.rs`, which must declare this module before
//! them: step registration happens at macro-expansion time, so a binding
//! expanded first would not yet see these definitions. The fixture repository
//! and the helpers are in [`fixture`].
//!
//! Nothing here asserts on git's own message text: the only diagnostic the
//! scenarios read is this tool's wrapper wording, per AX-GIT-NLS.

use std::{ffi::OsStr, fs};

use rstest_bdd_macros::{given, then, when};

// The path is stated because this module is itself loaded through a `#[path]`
// attribute, which leaves Rust looking for a child module beside this file
// rather than in a directory named after it.
#[path = "git_selection/fixture.rs"]
mod fixture;

// Re-exported because the scenario bindings name the state type, and they live
// in the test crate's root module rather than beside the steps.
pub use fixture::GitSelectionState;
use fixture::{
    CLEAN,
    RAGGED,
    before_bytes,
    commit_file,
    git,
    git_raw,
    last_run,
    repo_path,
    run_directory,
    run_once,
    snapshot,
    write_fixture,
};

#[given("a Git repository containing a committed file {name:string} with a broken table")]
fn repository_with_committed_file(state: &GitSelectionState, name: String) {
    let repo = repo_path(state);
    fs::write(repo.join(".gitignore"), "build/\n").expect("write .gitignore");
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["add", "--", ".gitignore"]);
    git(
        &repo,
        &["commit", "-m", "initialise the fixture repository"],
    );

    commit_file(state, &name, RAGGED);
}

#[given("a committed file {name:string} with a broken table")]
fn committed_file(state: &GitSelectionState, name: String) { commit_file(state, &name, RAGGED); }

#[given("an untracked file {name:string} with a broken table")]
fn untracked_file(state: &GitSelectionState, name: String) {
    write_fixture(&repo_path(state).join(&name), RAGGED);
}

#[given("an ignored file {name:string} with a broken table")]
fn ignored_file(state: &GitSelectionState, name: String) {
    let repo = repo_path(state);
    write_fixture(&repo.join(&name), RAGGED);
    // `git check-ignore` exits zero precisely when the path is ignored, so this
    // asserts the premise: a scenario that claims to exclude an ignored file
    // proves nothing if its fixture never made one ignored.
    assert!(
        git_raw(&repo, &["check-ignore", "--quiet", "--", &name])
            .status
            .success(),
        "{name} must be ignored, or the scenario proves nothing"
    );
}

/// A tracked symbolic link, which selection must never follow.
///
/// Unix only: on Windows `git` checks a symlink out as a plain file holding the
/// target's path, so the scenario has no subject there. Both this step and its
/// scenario binding are gated together, so the registry and the feature file
/// stay consistent.
#[cfg(unix)]
#[given("a committed symlink {name:string} pointing at {target:string}")]
fn committed_symlink(state: &GitSelectionState, name: String, target: String) {
    let repo = repo_path(state);
    let path = repo.join(&name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create the fixture's directory");
    }
    std::os::unix::fs::symlink(&target, &path).expect("create the fixture symlink");
    git(&repo, &["add", "--", &name]);
    git(&repo, &["commit", "-m", &format!("add {name}")]);
}

#[given("the file {name:string} is deleted from the working tree")]
fn deleted_from_working_tree(state: &GitSelectionState, name: String) {
    fs::remove_file(repo_path(state).join(&name)).expect("delete the fixture");
}

/// Makes every candidate beneath `name` unreadable, without a permission trick.
///
/// Replacing the directory with a regular file leaves the index listing
/// `docs/guide.md` while the working tree cannot resolve it: `symlink_metadata`
/// fails with "not a directory", which is a failure to classify rather than an
/// absence. A permission-based fixture would behave the same way for an
/// unprivileged user and differently for a privileged one, which is why this
/// one does not use permissions.
#[given("the directory {name:string} is replaced by a regular file")]
fn directory_replaced_by_a_file(state: &GitSelectionState, name: String) {
    let path = repo_path(state).join(&name);
    fs::remove_dir_all(&path).expect("remove the fixture directory");
    fs::write(&path, "not a directory\n").expect("write the fixture file");
}

/// Leaves the repository mid-merge with `name` conflicted.
///
/// The conflict is real rather than staged: two branches change the same line
/// of `name` differently, and `git merge` is allowed to fail, which is how
/// `MERGE_HEAD` comes to exist. The conflicted file therefore carries the three
/// marker forms because git wrote them, not because the fixture did.
///
/// Both sides are ragged rather than formatted, so that a permitted rewrite is
/// observable in the file's bytes: a fixture of already-formatted content would
/// make the scenario pass whether or not anything was written.
#[given("an unresolved merge conflict in the tracked file {name:string}")]
fn unresolved_merge_conflict(state: &GitSelectionState, name: String) {
    let repo = repo_path(state);
    git(&repo, &["branch", "side"]);
    write_fixture(&repo.join(&name), "|A|B|\n|---|---|\n|1|9|\n");
    git(&repo, &["commit", "-am", "the main side"]);
    git(&repo, &["checkout", "--quiet", "side"]);
    write_fixture(&repo.join(&name), "|A|B|\n|---|---|\n|1|3|\n");
    git(&repo, &["commit", "-am", "the side branch"]);
    git(&repo, &["checkout", "--quiet", "main"]);

    let merge = git_raw(&repo, &["merge", "side"]);
    assert!(
        !merge.status.success(),
        "the fixture's merge must conflict for the scenario to mean anything"
    );
    let conflicted = fs::read_to_string(repo.join(&name)).expect("read the conflicted fixture");
    for marker in ["<<<<<<<", "=======", ">>>>>>>"] {
        assert!(
            conflicted.lines().any(|line| line.starts_with(marker)),
            "git must have written a {marker} marker at the start of a line: {conflicted:?}"
        );
    }
}

#[given("every tracked Markdown file is already formatted")]
fn every_tracked_file_is_formatted(state: &GitSelectionState) {
    commit_file(state, "docs/guide.md", CLEAN);
}

/// Leaves the repository holding `name` and the ignore file, and nothing else.
///
/// The working tree is emptied first, so this step is the repository's whole
/// contents rather than an addition to the `Background` it follows.
#[given("a Git repository containing only the committed file {name:string}")]
fn repository_containing_only(state: &GitSelectionState, name: String) {
    let repo = repo_path(state);
    for entry in fs::read_dir(&repo).expect("read the fixture repository") {
        let entry = entry.expect("read a fixture directory entry");
        if entry.file_name() == OsStr::new(".git") {
            continue;
        }
        let path = entry.path();
        let removed = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        removed.expect("empty the working tree");
    }
    fs::write(repo.join(".gitignore"), "build/\n").expect("write .gitignore");
    write_fixture(&repo.join(&name), RAGGED);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-m", "only the named file"]);
}

#[given("the working directory is not inside a Git repository")]
fn outside_a_repository(state: &GitSelectionState) {
    let elsewhere = state
        .elsewhere
        .get_or_insert_with(|| tempfile::tempdir().expect("create temporary directory"));
    state.run_dir.set(elsewhere.path().to_path_buf());
}

#[when("I run mdtablefix with {flags:string}")]
fn runs_with(state: &GitSelectionState, flags: String) {
    let directory = run_directory(state);
    state.before.set(snapshot(&directory));
    state.run.set(run_once(&directory, &flags));
}

#[when("I run mdtablefix from {directory:string} with {flags:string}")]
fn runs_from(state: &GitSelectionState, directory: String, flags: String) {
    let root = repo_path(state);
    let nested = root.join(&directory);
    state.before.set(snapshot(&root));
    state.run.set(run_once(&nested, &flags));
}

#[then("the command succeeds")]
fn command_succeeds(state: &GitSelectionState) {
    let run = last_run(state);
    assert_eq!(
        run.status, 0,
        "the command must succeed, stderr: {}",
        run.stderr
    );
}

#[then("the command fails")]
fn command_fails(state: &GitSelectionState) {
    let run = last_run(state);
    assert_ne!(
        run.status, 0,
        "the command must fail, stdout: {}",
        run.stdout
    );
}

#[then("the exit status is {expected:i32}")]
fn exit_status_is(state: &GitSelectionState, expected: i32) {
    let run = last_run(state);
    assert_eq!(run.status, expected, "exit status, stderr: {}", run.stderr);
}

#[then("the command exits with status {expected:i32}")]
fn command_exits_with_status(state: &GitSelectionState, expected: i32) {
    exit_status_is(state, expected);
}

#[then("stdout is empty")]
fn stdout_is_empty(state: &GitSelectionState) {
    let run = last_run(state);
    assert!(
        run.stdout.is_empty(),
        "standard output was {:?}",
        run.stdout
    );
}

#[then("stdout is exactly {expected:string}")]
fn stdout_is(state: &GitSelectionState, expected: String) {
    let run = last_run(state);
    assert_eq!(run.stdout, format!("{expected}\n"), "standard output");
}

#[then("stdout names {name:string}")]
fn stdout_names(state: &GitSelectionState, name: String) {
    let run = last_run(state);
    assert!(
        run.stdout.contains(&name),
        "standard output must name {name}: {:?}",
        run.stdout
    );
}

/// Standard output carries `text`, without pinning the rest of it.
///
/// Separate from [`stdout_names`] and from the exact rendering because the two
/// reporting modes that use it print a whole document or a diff: the assertion
/// is about the payload being there, and the surrounding framing is not this
/// step's subject.
#[then("stdout contains {text:string}")]
fn stdout_contains(state: &GitSelectionState, text: String) {
    let run = last_run(state);
    assert!(
        run.stdout.contains(&text),
        "standard output must contain {text:?}: {:?}",
        run.stdout
    );
}

#[then("stderr contains {text:string}")]
fn stderr_contains(state: &GitSelectionState, text: String) {
    let run = last_run(state);
    assert!(
        run.stderr.contains(&text),
        "standard error must contain {text:?}: {:?}",
        run.stderr
    );
}

/// Standard input was not read.
///
/// The run supplies a document on standard input, so a tool that fell through
/// to it would print that document. An empty standard output is therefore
/// evidence of the property rather than a coincidence, and no timeout is
/// needed to observe it: the pipe is closed after the write, so a read would
/// return the document rather than block.
#[then("standard input was not read")]
fn standard_input_was_not_read(state: &GitSelectionState) {
    let run = last_run(state);
    assert!(
        run.stdout.is_empty(),
        "standard input carried a document mdtablefix would have printed had it read standard \
         input: {:?}",
        run.stdout
    );
}

#[then("the file {name:string} has a reflowed table")]
fn file_has_a_reflowed_table(state: &GitSelectionState, name: String) {
    let path = repo_path(state).join(&name);
    let content = fs::read_to_string(&path).expect("read the rewritten fixture");
    assert_eq!(content, CLEAN, "{name} must hold the formatted table");
    assert_ne!(
        content.as_bytes(),
        before_bytes(state, &name).as_slice(),
        "{name} must actually have changed, or the assertion is vacuous"
    );
}

/// The rewrite a permitted conflict leaves, markers and all.
///
/// Both halves are needed, and neither alone would do: a refusal satisfies the
/// marker assertion, and a rewrite that discarded the markers satisfies the
/// byte comparison, so the scenario would pass for two implementations that
/// are not the one it specifies. The markers are counted rather than merely
/// found, because a marker run shorter than the seven characters the guard
/// reads is no longer a conflict marker at all.
#[then("the file {name:string} is rewritten with its conflict markers intact")]
fn rewritten_with_markers(state: &GitSelectionState, name: String) {
    let path = repo_path(state).join(&name);
    let content = fs::read(&path).expect("read the rewritten fixture");
    assert_ne!(
        content,
        before_bytes(state, &name),
        "{name} must actually have been rewritten, or the scenario proves nothing"
    );

    let text = String::from_utf8(content).expect("the rewritten fixture is UTF-8");
    for marker in ["<<<<<<<", "=======", ">>>>>>>"] {
        let lines = text.lines().filter(|line| line.starts_with(marker)).count();
        assert_eq!(
            lines, 1,
            "{name} must still carry exactly one {marker} line: {text:?}"
        );
    }
}

#[then("the file {name:string} is unchanged")]
fn file_is_unchanged(state: &GitSelectionState, name: String) {
    let path = repo_path(state).join(&name);
    let content = fs::read(&path).expect("read the fixture");
    let expected = before_bytes(state, &name);
    assert_eq!(
        content,
        expected,
        "{name} must be byte-identical:\nactual:   {:?}\nexpected: {:?}",
        String::from_utf8_lossy(&content),
        String::from_utf8_lossy(&expected)
    );
}

#[then("the file {name:string} is still a symlink")]
fn file_is_still_a_symlink(state: &GitSelectionState, name: String) {
    let path = repo_path(state).join(&name);
    let metadata = fs::symlink_metadata(&path).expect("read the fixture's metadata");
    assert!(
        metadata.file_type().is_symlink(),
        "{name} must remain a symbolic link, not be replaced by a regular file"
    );
}
