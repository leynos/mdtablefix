//! Integration tests for `--check`: argument order, the exit-status contract,
//! and the read-only guarantee.
//!
//! These three cases read badly as prose, which is why they are not in
//! `tests/features/check_mode.feature`: the ordering batch needs eight files
//! whose alphabetical, size, and completion orders all differ from argument
//! order, the exit-status contract is a cross product, and the read-only
//! guarantee is asserted against a directory snapshot.
//!
//! `--in-place` and `--diff` are exercised here too, because the contract that
//! separates them from `--check` — drift is reported, not failed, and it is
//! reported by both reporting modes rather than only by the terse one — is only
//! meaningful beside the modes it separates. The cross product lives in one
//! place so that a mode cannot be added to it partially.

use std::{ffi::OsString, fs, path::Path, time::SystemTime};

use assert_cmd::Command;

#[path = "cli_check/arguments.rs"]
mod arguments;
#[path = "cli_check/closed_pipe.rs"]
mod closed_pipe;
#[path = "cli_check/exit_status.rs"]
mod exit_status;
#[path = "cli_check/no_write.rs"]
mod no_write;
#[path = "cli_check/ordering.rs"]
mod ordering;

/// A ragged table that every mode must agree needs reformatting.
const RAGGED: &str = "|A|B|\n|---|---|\n|1|2|\n";

/// The same table already aligned, which no mode may change.
///
/// These are the formatter's own bytes, not a hand-written approximation:
/// every cell is padded to the delimiter row's width, so `| A | B |` would
/// itself be reported as drift. `tests/line_endings.rs` holds the same
/// fixture.
const CLEAN: &str = "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n";

/// The line delta of one ragged table, as `git diff --numstat` reports it.
const RAGGED_INSERTIONS: i64 = 3;

/// The matching deletion count.
const RAGGED_DELETIONS: i64 = 3;

/// Runs the binary with raw arguments in `directory` and returns its output.
///
/// The arguments are [`OsString`] rather than `&str` because a path on the
/// command line need not be valid UTF-8, and what the tool does with such a path
/// is part of its contract rather than an accident of the test helper.
fn run_in_os(directory: &Path, args: &[OsString]) -> std::process::Output {
    Command::cargo_bin("mdtablefix")
        .expect("cargo binary")
        .current_dir(directory)
        .args(args)
        .output()
        .expect("run mdtablefix")
}

/// Runs the binary with `args` in `directory` and returns its raw output.
///
/// Paths are passed relative to `directory`, which is also the working
/// directory, so report lines name the files as the user wrote them rather
/// than by an absolute path the temporary directory invented.
fn run_in(directory: &Path, args: &[&str]) -> std::process::Output {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    run_in_os(directory, &args)
}

/// The captured standard output as text.
fn stdout_of(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

/// The captured standard error as text.
fn stderr_of(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

/// The exit status, or `-1` if the process was signalled.
fn status_of(output: &std::process::Output) -> i32 { output.status.code().unwrap_or(-1) }

/// A file's observable identity: name, length, and modification time.
type Snapshot = Vec<(String, u64, Option<SystemTime>)>;

/// Captures the directory's entry set, lengths, and modification times.
fn snapshot(directory: &Path) -> Snapshot {
    let mut entries: Snapshot = fs::read_dir(directory)
        .expect("read directory")
        .map(|entry| {
            let entry = entry.expect("read directory entry");
            let metadata = entry.metadata().expect("read metadata");
            (
                entry.file_name().to_string_lossy().into_owned(),
                metadata.len(),
                metadata.modified().ok(),
            )
        })
        .collect();
    entries.sort();
    entries
}
