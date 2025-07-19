//! Integration tests for remaining CLI behaviour.
//!
//! Covers file handling with `--in-place` and ellipsis replacement.

use std::{fs::File, io::Write};

use rstest::{fixture, rstest};
use tempfile::tempdir;

mod prelude;
use prelude::*;

#[fixture]
fn broken_table() -> Vec<String> {
    vec![
        "| A | B |    |".to_string(),
        "| 1 | 2 |  | 3 | 4 |".to_string(),
    ]
}

#[test]
/// Verifies that the CLI fails when the `--in-place` flag is used without specifying a file.
///
/// This test ensures that running `mdtablefix --in-place` without a file argument results in a
/// command failure.
fn test_cli_in_place_requires_file() {
    Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--in-place")
        .assert()
        .failure();
}

#[rstest]
/// Tests that the CLI processes a file containing a broken Markdown table and outputs the corrected
/// table to stdout.
///
/// This test creates a temporary file with a malformed table, runs the `mdtablefix` binary on it,
/// and asserts that the output is the expected fixed table.
fn test_cli_process_file(broken_table: Vec<String>) {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("sample.md");
    let mut f = File::create(&file_path).unwrap();
    for line in &broken_table {
        writeln!(f, "{line}").unwrap();
    }
    f.flush().unwrap();
    drop(f);
    Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg(&file_path)
        .assert()
        .success()
        .stdout("| A | B |\n| 1 | 2 |\n| 3 | 4 |\n");
}

#[test]
fn test_cli_ellipsis_option() {
    let output = Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--ellipsis")
        .write_stdin("foo...\n")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "foo…\n");
}

#[test]
fn test_cli_ellipsis_code_span() {
    let output = Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--ellipsis")
        .write_stdin("before `dots...` after\n")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "before `dots...` after\n"
    );
}

#[test]
fn test_cli_ellipsis_fenced_block() {
    let output = Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--ellipsis")
        .write_stdin("```\nlet x = ...;\n```\n")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "```\nlet x = ...;\n```\n"
    );
}

#[test]
fn test_cli_ellipsis_long_sequence() {
    let output = Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--ellipsis")
        .write_stdin("wait....\n")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "wait….\n");
}

#[test]
fn test_cli_ellipsis_multiple_sequences() {
    let output = Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--ellipsis")
        .write_stdin("First... then second... done.\n")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "First… then second… done.\n"
    );
}
