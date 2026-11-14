//! The fake prover-tools runner the Verus harness drives `make` with.
//!
//! The runner is a script this process writes, so it is run through `bash`
//! rather than executed directly (#586). The script lives in the shared
//! [`TestDir`] boundary from `tests/common/fs.rs`: writes and reads go through
//! its directory capability, and an absolute path is derived only for the two
//! process boundaries that need one — `make`, which receives the runner and
//! log paths through the environment, and the working directory of the child.

use std::process::Command;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use cap_std::{
    fs::{Permissions, PermissionsExt},
    fs_utf8::Dir,
};

use super::{manifest_dir, test_fs::TestDir};

/// A fake runner script in its own temporary directory, with its call log.
pub struct FakeProverTools {
    directory: TestDir,
    smoke_mode: &'static str,
}

/// The fake runner's script, one branch per gate it answers.
///
/// Kept out of [`fake_prover_tools`] so that function reads as what it does —
/// write the script, make it executable, hand back its paths — instead of as
/// two hundred lines of shell. Each branch answers the proof file the gate
/// hands it, keyed by the gate's mirror directory, so the harness can inject
/// exactly the transcript that gate is meant to judge, including the ones it
/// must refuse.
const RUNNER_SCRIPT: &str = include_str!("fixtures/fake_prover_tools_runner.sh");

/// The runner's file name, which the script's own log line reports.
const RUNNER: &str = "prover-tools";

/// The file the runner appends each invocation to.
const LOG: &str = "prover-tools.log";

impl FakeProverTools {
    /// The absolute path of the runner, for the child process that runs it.
    pub fn path(&self) -> Utf8PathBuf { self.directory.path().join(RUNNER) }

    /// The directory capability holding the script and its log.
    pub fn directory(&self) -> &Dir { self.directory.directory() }
}

/// Writes a fake runner whose smoke proof behaves as `smoke_mode` says.
pub fn fake_prover_tools(smoke_mode: &'static str) -> Result<FakeProverTools> {
    let directory = TestDir::new().context("create fake prover-tools directory")?;
    let handle = directory.directory();
    handle
        .write(RUNNER, RUNNER_SCRIPT)
        .context("write fake prover-tools runner")?;
    handle
        .set_permissions(RUNNER, Permissions::from_mode(0o755))
        .context("make fake prover-tools runner executable")?;
    Ok(FakeProverTools {
        directory,
        smoke_mode,
    })
}

/// Returns the command line that runs the fake runner through `bash`.
///
/// The runner is written by this process, and any other test thread that
/// forks while the file is open for writing leaves a child holding a write
/// descriptor until it execs. Executing the file directly in that window
/// fails with `ETXTBSY` (#586), so the harness runs `bash` and lets it read
/// the script instead of executing a file it has just written. The path is
/// single-quoted, because Make hands the value to a shell.
fn runner_command(runner: &FakeProverTools) -> String {
    format!("bash '{}'", runner.path().as_str().replace('\'', r"'\''"))
}

/// Builds a `make` invocation of `target` that uses the fake runner.
pub fn make_command(target: &str, runner: &FakeProverTools) -> Command {
    let mut command = Command::new("make");
    command
        .arg("--no-print-directory")
        .arg(target)
        .current_dir(manifest_dir())
        .env("PROVER_TOOLS", runner_command(runner))
        .env("FAKE_PROVER_TOOLS_LOG", runner.path().with_file_name(LOG))
        .env("FAKE_PROVER_TOOLS_SMOKE_MODE", runner.smoke_mode);
    command
}

/// Reads the arguments the fake runner was called with, one call per line.
pub fn runner_log(runner: &FakeProverTools) -> Result<String> {
    runner
        .directory()
        .read_to_string(LOG)
        .context("read fake prover-tools log")
}
