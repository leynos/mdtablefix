//! The fake prover-tools runner the Verus harness drives `make` with.
//!
//! The runner is a script this process writes, so it is run through `bash`
//! rather than executed directly (#586).

use std::process::Command;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use cap_std::fs::{Permissions, PermissionsExt};
use tempfile::TempDir;

use super::{manifest_dir, open_dir, utf8};

/// A fake runner script in its own temporary directory, with its call log.
pub struct FakeProverTools {
    _directory: TempDir,
    pub path: Utf8PathBuf,
    log_path: Utf8PathBuf,
    smoke_mode: &'static str,
}

/// Writes a fake runner whose smoke proof behaves as `smoke_mode` says.
pub fn fake_prover_tools(smoke_mode: &'static str) -> Result<FakeProverTools> {
    let directory = TempDir::new().context("create fake prover-tools directory")?;
    let root = utf8(directory.path())?;
    let handle = open_dir(root)?;
    let path = root.join("prover-tools");
    let log_path = root.join("prover-tools.log");
    let script = r#"#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "${FAKE_PROVER_TOOLS_LOG:?}"
if [[ "$*" == "verus run --repo-root . --proof-file verus/smoke.rs" ]]; then
    case "${FAKE_PROVER_TOOLS_SMOKE_MODE:?}" in
        rejected)
            echo "Verus proofs failed"
            exit 1
            ;;
        accepted) exit 0 ;;
        unrelated_failure)
            echo "runner unavailable"
            exit 1
            ;;
    esac
fi
"#;
    handle
        .write("prover-tools", script)
        .context("write fake prover-tools runner")?;
    handle
        .set_permissions("prover-tools", Permissions::from_mode(0o755))
        .context("make fake prover-tools runner executable")?;
    Ok(FakeProverTools {
        _directory: directory,
        path,
        log_path,
        smoke_mode,
    })
}

/// Returns the command line that runs the fake runner through `bash`.
///
/// The runner is written by this process, and any other test thread that
/// forks while the file is open for writing leaves a child holding a write
/// descriptor until it execs. Executing the file directly in that window
/// fails with `ETXTBSY` (#586), so the harness runs `bash` and lets it read
/// the script instead of executing a file it has just written.
fn runner_command(runner: &FakeProverTools) -> String { format!("bash {}", runner.path) }

/// Builds a `make` invocation of `target` that uses the fake runner.
pub fn make_command(target: &str, runner: &FakeProverTools) -> Command {
    let mut command = Command::new("make");
    command
        .arg("--no-print-directory")
        .arg(target)
        .current_dir(manifest_dir())
        .env("PROVER_TOOLS", runner_command(runner))
        .env(
            "VERUS_RUN",
            format!("{} verus run --repo-root .", runner_command(runner)),
        )
        .env("FAKE_PROVER_TOOLS_LOG", &runner.log_path)
        .env("FAKE_PROVER_TOOLS_SMOKE_MODE", runner.smoke_mode);
    command
}

/// Reads the arguments the fake runner was called with, one call per line.
pub fn runner_log(runner: &FakeProverTools) -> Result<String> {
    let root = runner
        .path
        .parent()
        .context("fake prover-tools path has no parent")?;
    open_dir(root)?
        .read_to_string("prover-tools.log")
        .context("read fake prover-tools log")
}
