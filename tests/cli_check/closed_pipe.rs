//! A closed pipe is an early exit rather than a panic.

use std::fs;

use tempfile::tempdir;

use super::RAGGED;

/// How many fixtures [`a_closed_pipe_is_a_successful_early_exit`] writes, and
/// how much padding each name carries.
///
/// The pair is chosen per platform because the limits that bound it pull in
/// opposite directions. A `--check` report is one line per file and that line
/// is the path the argument named, so the arguments *are* the output, and the
/// run only stays blocked on a full pipe — which is what makes the early exit
/// deterministic rather than a race — when the names are long enough to write
/// past what the pipe holds. Unix has a 64 KiB pipe and room for a 93 KiB
/// command line; Windows refuses a command line past 32 767 characters with
/// `ERROR_FILENAME_EXCED_RANGE`, so the Unix pair cannot even be spawned there,
/// and its pipe is created with a size hint of zero — the system default, not
/// the Unix 64 KiB — so a far smaller run fills it.
#[cfg(unix)]
const CLOSED_PIPE_FILES: usize = 500;
#[cfg(unix)]
const CLOSED_PIPE_PADDING: usize = 180;
#[cfg(not(unix))]
const CLOSED_PIPE_FILES: usize = 250;
#[cfg(not(unix))]
const CLOSED_PIPE_PADDING: usize = 80;

/// A closed pipe must not turn into a panic, which would exit `101` — a status
/// the tool does not document.
///
/// This is the plan's `mdtablefix --check *.md | head` shape. Every file
/// drifts, so the run would exit `1` if it completed: exit `0` therefore means
/// the write failed and the run stopped early, and a run that panicked instead
/// exits `101` or is killed by a signal. There are enough reports to overflow a
/// pipe buffer, so the child is still writing — and blocked — when the read end
/// closes, which is what makes the failure deterministic rather than a race.
/// [`CLOSED_PIPE_FILES`] and [`CLOSED_PIPE_PADDING`] carry the size and why it
/// differs by platform.
#[test]
fn a_closed_pipe_is_a_successful_early_exit() {
    use std::{
        io::Read as _,
        process::{Command, Stdio},
    };

    let dir = tempdir().expect("create temporary directory");
    let names: Vec<String> = (0..CLOSED_PIPE_FILES)
        .map(|index| format!("{index:0>3}-{}.md", "p".repeat(CLOSED_PIPE_PADDING)))
        .collect();
    for name in &names {
        fs::write(dir.path().join(name), RAGGED).expect("write fixture");
    }

    let mut child = Command::new(env!("CARGO_BIN_EXE_mdtablefix"))
        .arg("--check")
        .args(&names)
        .current_dir(dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn mdtablefix");

    let mut stdout = child.stdout.take().expect("child stdout is piped");
    let mut first = [0u8; 1];
    stdout.read_exact(&mut first).expect("read one byte");
    drop(stdout);

    let output = child.wait_with_output().expect("wait for mdtablefix");
    let stderr = String::from_utf8_lossy(&output.stderr);

    // On Unix the reports are past what the pipe holds, so the child is blocked
    // in `write` when the read end closes and cannot have completed the run.
    // Windows cannot be held to the same standard: its reports are bounded by
    // the command-line cap described on the constants above, so the child may
    // finish before the parent's read end closes, and `1` — drift found — is a
    // documented status rather than a defect. A panic or a crash is not
    // documented on either platform.
    #[cfg(unix)]
    assert_eq!(
        output.status.code(),
        Some(0),
        "a closed pipe is an early exit, not a failure: {stderr}"
    );
    #[cfg(not(unix))]
    assert!(
        matches!(output.status.code(), Some(0 | 1)),
        "a closed pipe is an early exit or a completed run, never an undocumented status: {stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "the write failure is handled, not panicked on: {stderr}"
    );
}
