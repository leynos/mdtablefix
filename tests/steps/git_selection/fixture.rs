//! The fixture repository, and the helpers every step builds on.
//!
//! Split from the step definitions so that neither file has to hold the whole
//! suite: this one is the fixture's environment, and `git_selection.rs` is the
//! grammar of the scenarios.
//!
//! Two pieces of the environment are neutralised rather than inherited. The
//! fixture repository sets `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_GLOBAL`, and
//! `HOME`, because the developer's own `core.excludesFile` would otherwise leak
//! into the selection the scenarios assert on — which is exactly the ambient
//! input CON-SAFE-001 exists to control. And `git commit` needs an identity,
//! which `GIT_CONFIG_GLOBAL=/dev/null` removes, so the fixture supplies one
//! through the environment instead of a configuration file it would then have
//! to clean up.

use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use assert_cmd::Command;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
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
/// without a mutable borrow crossing a step boundary. The steps live in the
/// parent module, so the slots they fill are visible to the crate; `repo` is
/// reached through [`repo_path`] alone and stays private.
#[derive(Default, ScenarioState)]
pub struct GitSelectionState {
    /// The fixture repository, created by the first `Given`.
    repo: Slot<TempDir>,
    /// A directory outside any repository, for the scenario that needs one.
    pub(crate) elsewhere: Slot<TempDir>,
    /// The directory the command runs in, when it is not the repository root.
    pub(crate) run_dir: Slot<PathBuf>,
    /// Every fixture file's bytes immediately before the run.
    pub(crate) before: Slot<Vec<(String, Vec<u8>)>>,
    /// The most recent run.
    pub(crate) run: Slot<Run>,
}

/// The fixture repository's root, created on first use.
///
/// The directory is created lazily rather than by a dedicated step so that the
/// first `Given` in a scenario decides what kind of repository it is: that step
/// writes the ignore file and runs `git init` into the directory this returns.
/// A `Slot` is filled through `get_or_insert_with` because `get` requires
/// `T: Clone`, which `TempDir` is not.
pub fn repo_path(state: &GitSelectionState) -> PathBuf {
    state
        .repo
        .get_or_insert_with(|| tempfile::tempdir().expect("create temporary directory"))
        .path()
        .to_path_buf()
}

/// The directory the command runs in, which is the repository root unless a
/// step moved it.
pub fn run_directory(state: &GitSelectionState) -> PathBuf {
    state.run_dir.get().unwrap_or_else(|| repo_path(state))
}

/// Runs `git` in `directory` with the fixture's hardened environment, returning
/// the raw output without judging it.
///
/// A failed `git` is a legitimate result here: the fixture's merge is expected
/// to conflict, and that is how the conflict is created rather than mocked.
pub fn git_raw(directory: &Path, args: &[&str]) -> std::process::Output {
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
pub fn git(directory: &Path, args: &[&str]) {
    let output = git_raw(directory, args);
    assert!(
        output.status.success(),
        "git {args:?} failed with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Creates the parent directories of `path` and writes `content` there.
pub fn write_fixture(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create the fixture's directory");
    }
    fs::write(path, content).expect("write a fixture");
}

/// Writes `name` with a broken table, stages it, and commits it.
pub fn commit_file(state: &GitSelectionState, name: &str, content: &str) {
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
pub fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
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
pub fn before_bytes(state: &GitSelectionState, name: &str) -> Vec<u8> {
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
pub fn last_run(state: &GitSelectionState) -> Run {
    state
        .run
        .get()
        .expect("the scenario must run mdtablefix before asserting on it")
}

/// Runs the binary once in `directory`, without recording it.
pub fn run_once(directory: &Path, flags: &str) -> Run {
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
