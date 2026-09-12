//! Step definitions for `tests/features/git_file_selection.feature`.
//!
//! Every step drives the real binary through `assert_cmd` and builds a real
//! repository with real `git`, so the feature specifies the command-line
//! contract rather than a reimplementation of it. The bindings themselves are
//! in `tests/git_file_selection.rs`, which must declare this module before
//! them: step registration happens at macro-expansion time, so a binding
//! expanded first would not yet see these definitions.
//!
//! Two pieces of the environment are neutralised rather than inherited. The
//! fixture repository sets `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_GLOBAL`, and
//! `HOME`, because the developer's own `core.excludesFile` would otherwise leak
//! into the selection the scenarios assert on — which is exactly the ambient
//! input CON-SAFE-001 exists to control. And `git commit` needs an identity,
//! which `GIT_CONFIG_GLOBAL=/dev/null` removes, so the fixture supplies one
//! through the environment instead of a configuration file it would then have
//! to clean up.
//!
//! Nothing here asserts on git's own message text: the only diagnostic the
//! scenarios read is this tool's wrapper wording, per AX-GIT-NLS.

use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use assert_cmd::Command;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, then, when};
use tempfile::TempDir;

/// A ragged table, which every mode must agree needs reformatting.
pub const RAGGED: &str = "|A|B|\n|---|---|\n|1|2|\n";

/// The same table already aligned, which no mode may change.
///
/// These are the formatter's own bytes: cells are padded to the delimiter row's
/// width, so `| A | B |` would itself be drift.
pub const CLEAN: &str = "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";

/// The name and address the fixture commits under.
///
/// `GIT_CONFIG_GLOBAL=/dev/null` leaves `git commit` with no `user.name`, and
/// the fixture must not write a configuration file the selection could then
/// read.
const IDENTITY_NAME: &str = "mdtablefix tests";
const IDENTITY_EMAIL: &str = "tests@example.invalid";

/// What one run of the binary produced.
#[derive(Clone, Debug)]
pub struct Run {
    /// The process exit status, or `-1` when the process was signalled.
    pub status: i32,
    /// Standard output as text.
    pub stdout: String,
    /// Standard error as text.
    pub stderr: String,
}

/// The state one scenario accumulates.
///
/// Each field is a `Slot`, so a step borrows the whole state immutably and
/// fills one slot, which is what lets `Given`, `When`, and `Then` share data
/// without a mutable borrow crossing a step boundary.
#[derive(Default, ScenarioState)]
pub struct GitSelectionState {
    /// The fixture repository, created by the first `Given`.
    repo: Slot<TempDir>,
    /// A directory outside any repository, for the scenario that needs one.
    elsewhere: Slot<TempDir>,
    /// The directory the command runs in, when it is not the repository root.
    run_dir: Slot<PathBuf>,
    /// Every fixture file's bytes immediately before the run.
    before: Slot<Vec<(String, Vec<u8>)>>,
    /// The most recent run.
    run: Slot<Run>,
}

/// The fixture repository's root, created on first use.
///
/// The directory is created lazily rather than by a dedicated step so that the
/// first `Given` in a scenario decides what kind of repository it is: that step
/// writes the ignore file and runs `git init` into the directory this returns.
/// A `Slot` is filled through `get_or_insert_with` because `get` requires
/// `T: Clone`, which `TempDir` is not.
fn repo_path(state: &GitSelectionState) -> PathBuf {
    state
        .repo
        .get_or_insert_with(|| tempfile::tempdir().expect("create temporary directory"))
        .path()
        .to_path_buf()
}

/// The directory the command runs in, which is the repository root unless a
/// step moved it.
fn run_directory(state: &GitSelectionState) -> PathBuf {
    state.run_dir.get().unwrap_or_else(|| repo_path(state))
}

/// Runs `git` in `directory` with the fixture's hardened environment, returning
/// the raw output without judging it.
///
/// A failed `git` is a legitimate result here: the fixture's merge is expected
/// to conflict, and that is how the conflict is created rather than mocked.
fn git_raw(directory: &Path, args: &[&str]) -> std::process::Output {
    ProcessCommand::new("git")
        .current_dir(directory)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("HOME", directory)
        .env("LC_ALL", "C")
        .env("LANGUAGE", "")
        .env("GIT_AUTHOR_NAME", IDENTITY_NAME)
        .env("GIT_AUTHOR_EMAIL", IDENTITY_EMAIL)
        .env("GIT_COMMITTER_NAME", IDENTITY_NAME)
        .env("GIT_COMMITTER_EMAIL", IDENTITY_EMAIL)
        .output()
        .expect("run git; the fixture needs it on PATH")
}

/// Runs `git` in `directory`, requiring it to succeed.
fn git(directory: &Path, args: &[&str]) {
    let output = git_raw(directory, args);
    assert!(
        output.status.success(),
        "git {args:?} failed with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Creates the parent directories of `path` and writes `content` there.
fn write_fixture(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create the fixture's directory");
    }
    fs::write(path, content).expect("write a fixture");
}

/// Writes `name` with a broken table, stages it, and commits it.
fn commit_file(state: &GitSelectionState, name: &str, content: &str) {
    let repo = repo_path(state);
    write_fixture(&repo.join(name), content);
    git(&repo, &["add", "--", name]);
    git(&repo, &["commit", "-m", &format!("add {name}")]);
}

/// Every fixture file under `root`, excluding `.git`, as a relative path paired
/// with its bytes.
///
/// A symbolic link is recorded as its target's text rather than followed, so a
/// rewrite through a link would show up twice: as a content change at the
/// target, and as a type change here. `.git` is excluded because `git ls-files`
/// may refresh the index it holds, which is not a change this tool made.
fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, directory: &Path, entries: &mut Vec<(String, Vec<u8>)>) {
        for entry in fs::read_dir(directory).expect("read the fixture directory") {
            let entry = entry.expect("read a fixture directory entry");
            let path = entry.path();
            let name = entry.file_name();
            if name == OsStr::new(".git") {
                continue;
            }
            // The key is spelled with `/` on every platform, because that is
            // how the feature file and the steps name their files; a Windows
            // separator here would make every lookup miss and read as a fixture
            // file that was never written.
            let relative = path
                .strip_prefix(root)
                .expect("every entry is beneath the root")
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let metadata = fs::symlink_metadata(&path).expect("read fixture metadata");
            if metadata.file_type().is_symlink() {
                let target = fs::read_link(&path).expect("read the link target");
                entries.push((relative, target.to_string_lossy().into_owned().into_bytes()));
            } else if metadata.is_dir() {
                walk(root, &path, entries);
            } else {
                entries.push((relative, fs::read(&path).expect("read a fixture")));
            }
        }
    }

    let mut entries = Vec::new();
    walk(root, root, &mut entries);
    entries.sort();
    entries
}

/// The bytes the fixture recorded for `name` before the run.
fn before_bytes(state: &GitSelectionState, name: &str) -> Vec<u8> {
    state
        .before
        .get()
        .expect("the run must capture the fixture's bytes first")
        .into_iter()
        .find(|(path, _)| path == name)
        .unwrap_or_else(|| panic!("{name} was not part of the fixture before the run"))
        .1
}

/// The most recent run, which every `Then` step reads.
fn last_run(state: &GitSelectionState) -> Run {
    state
        .run
        .get()
        .expect("the scenario must run mdtablefix before asserting on it")
}

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

/// Leaves the repository mid-merge with `name` conflicted.
///
/// The conflict is real rather than staged: two branches change the same line
/// of `name` differently, and `git merge` is allowed to fail, which is how
/// `MERGE_HEAD` comes to exist. The conflicted file therefore carries the three
/// marker forms because git wrote them, not because the fixture did.
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

/// Runs the binary once in `directory`, without recording it.
fn run_once(directory: &Path, flags: &str) -> Run {
    let output = Command::cargo_bin("mdtablefix")
        .expect("cargo binary")
        .current_dir(directory)
        .args(flags.split_whitespace())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("HOME", directory)
        .env("LC_ALL", "C")
        .env("LANGUAGE", "")
        // Standard input carries a document the tool would print had it read
        // it, which is what makes "standard input was not read" observable
        // rather than assumed: the assertion is that standard output is empty.
        .write_stdin(RAGGED)
        .output()
        .expect("run mdtablefix");

    Run {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
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
