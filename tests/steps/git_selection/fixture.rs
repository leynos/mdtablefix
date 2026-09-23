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

use anyhow::{Context, Result, anyhow, ensure};
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
/// The slot is inspected by reference because `get` requires `T: Clone`,
/// which `TempDir` is not. A new directory is stored only after creation
/// succeeds, so a failed first use leaves the slot empty.
pub fn repo_path(state: &GitSelectionState) -> Result<PathBuf> {
    if let Some(path) = state.repo.with_ref(|repo| repo.path().to_path_buf()) {
        return Ok(path);
    }
    let repo = tempfile::tempdir().context("create the fixture repository")?;
    let path = repo.path().to_path_buf();
    state.repo.set(repo);
    Ok(path)
}

/// The directory the command runs in, which is the repository root unless a
/// step moved it.
pub fn run_directory(state: &GitSelectionState) -> Result<PathBuf> {
    state.run_dir.get().map_or_else(|| repo_path(state), Ok)
}

/// Runs `git` in `directory` with the fixture's hardened environment, returning
/// the raw output without judging it.
///
/// A failed `git` is a legitimate result here: the fixture's merge is expected
/// to conflict, and that is how the conflict is created rather than mocked.
pub fn git_raw(directory: &Path, args: &[&str]) -> Result<std::process::Output> {
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
        .with_context(|| format!("run git {args:?} in {}", directory.display()))
}

/// Runs `git` in `directory`, requiring it to succeed.
pub fn git(directory: &Path, args: &[&str]) -> Result<()> {
    let output = git_raw(directory, args)?;
    ensure!(
        output.status.success(),
        "git {args:?} failed with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// Creates the parent directories of `path` and writes `content` there.
pub fn write_fixture(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create fixture directory {}", parent.display()))?;
    }
    fs::write(path, content).with_context(|| format!("write fixture {}", path.display()))
}

/// Writes `name` with a broken table, stages it, and commits it.
pub fn commit_file(state: &GitSelectionState, name: &str, content: &str) -> Result<()> {
    let repo = repo_path(state)?;
    write_fixture(&repo.join(name), content)?;
    git(&repo, &["add", "--", name])?;
    git(&repo, &["commit", "-m", &format!("add {name}")])
}

/// Every fixture file under `root`, excluding `.git`, as a relative path paired
/// with its bytes.
///
/// A symbolic link is recorded as its target's text rather than followed, so a
/// rewrite through a link would show up twice: as a content change at the
/// target, and as a type change here. `.git` is excluded because `git ls-files`
/// may refresh the index it holds, which is not a change this tool made.
pub fn snapshot(root: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut entries = Vec::new();
    walk_snapshot(root, root, &mut entries)?;
    entries.sort();
    Ok(entries)
}

/// Walks one fixture directory and records its entries without following links.
fn walk_snapshot(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<(String, Vec<u8>)>,
) -> Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("read fixture directory {}", directory.display()))?
    {
        let entry = entry
            .with_context(|| format!("read entry in fixture directory {}", directory.display()))?;
        record_snapshot_entry(root, &entry, entries)?;
    }
    Ok(())
}

/// Records one entry, recursing only when it is a directory.
fn record_snapshot_entry(
    root: &Path,
    entry: &fs::DirEntry,
    entries: &mut Vec<(String, Vec<u8>)>,
) -> Result<()> {
    if entry.file_name() == OsStr::new(".git") {
        return Ok(());
    }
    let path = entry.path();
    // The key is spelled with `/` on every platform, because that is how the
    // feature file and steps name their files; a Windows separator here would
    // make every lookup miss and read as a fixture file never written.
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} is outside {}", path.display(), root.display()))?
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("read fixture metadata {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(&path)
            .with_context(|| format!("read fixture link {}", path.display()))?;
        entries.push((relative, target.to_string_lossy().into_owned().into_bytes()));
    } else if metadata.is_dir() {
        walk_snapshot(root, &path, entries)?;
    } else {
        let bytes = fs::read(&path).with_context(|| format!("read fixture {}", path.display()))?;
        entries.push((relative, bytes));
    }
    Ok(())
}

/// The bytes the fixture recorded for `name` before the run.
pub fn before_bytes(state: &GitSelectionState, name: &str) -> Result<Vec<u8>> {
    let before = state
        .before
        .get()
        .context("the run must capture the fixture's bytes first")?;
    before
        .into_iter()
        .find(|(path, _)| path == name)
        .map(|(_, bytes)| bytes)
        .ok_or_else(|| anyhow!("{name} was not part of the fixture before the run"))
}

/// The most recent run, which every `Then` step reads.
pub fn last_run(state: &GitSelectionState) -> Result<Run> {
    state
        .run
        .get()
        .context("the scenario must run mdtablefix before asserting on it")
}

/// Runs the binary once in `directory`, without recording it.
pub fn run_once(directory: &Path, flags: &str) -> Result<Run> {
    let output = Command::cargo_bin("mdtablefix")
        .context("locate the mdtablefix test binary")?
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
        .with_context(|| format!("run mdtablefix in {}", directory.display()))?;

    Ok(Run {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
