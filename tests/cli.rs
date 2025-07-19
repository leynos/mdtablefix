//! Integration tests for CLI interface behaviour of the `mdtablefix` tool.
//!
//! This module validates the command-line interface functionality, including:
//! - File handling with the `--in-place` flag
//! - Ellipsis replacement with the `--ellipsis` option
//! - Error handling for invalid argument combinations
//! - Processing of Markdown files through the CLI interface

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

/// Verifies that the CLI fails when the `--in-place` flag is used without specifying a file.
///
/// This test ensures that running `mdtablefix --in-place` without a file argument results in a
/// command failure.
#[test]
fn test_cli_in_place_requires_file() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--in-place")
        .assert()
        .failure();
}

/// Tests that the CLI processes a file containing a broken Markdown table and outputs the corrected
/// table to stdout.
///
/// This test creates a temporary file with a malformed table, runs the `mdtablefix` binary on it,
/// and asserts that the output is the expected fixed table.
#[rstest]
fn test_cli_process_file(broken_table: Vec<String>) {
    let dir = tempdir().expect("failed to create temporary directory");
    let file_path = dir.path().join("sample.md");
    let mut f = File::create(&file_path).expect("failed to create temporary file");
    for line in &broken_table {
        writeln!(f, "{line}").expect("failed to write line");
    }
    f.flush().expect("failed to flush file");
    drop(f);
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg(&file_path)
        .assert()
        .success()
        .stdout("| A | B |\n| 1 | 2 |\n| 3 | 4 |\n");
}

/// Tests that the `--ellipsis` option replaces triple dots with a Unicode ellipsis character.
///
/// Verifies that the CLI correctly processes input containing "..." and outputs "…".
#[test]
fn test_cli_ellipsis_option() {
    let output = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("foo...\n")
        .output()
        .expect("Failed to execute mdtablefix command");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "foo…\n");
}

/// Tests that the `--ellipsis` option preserves dots within inline code spans.
///
/// Verifies that triple dots inside backtick-delimited code spans are not converted to ellipsis.
#[test]
fn test_cli_ellipsis_code_span() {
    let output = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("before `dots...` after\n")
        .output()
        .expect("Failed to execute mdtablefix command");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "before `dots...` after\n"
    );
}

/// Tests that the `--ellipsis` option does not alter fenced code blocks.
///
/// Ensures that sequences like "..." inside a fenced code block remain unchanged.
#[test]
fn test_cli_ellipsis_fenced_block() {
    let output = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("```\nlet x = ...;\n```\n")
        .output()
        .expect("Failed to execute mdtablefix command");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "```\nlet x = ...;\n```\n"
    );
}

/// Tests ellipsis replacement for sequences longer than three characters.
///
/// Confirms that only the first three dots are replaced with an ellipsis.
#[test]
fn test_cli_ellipsis_long_sequence() {
    let output = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("wait....\n")
        .output()
        .expect("Failed to execute mdtablefix command");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "wait….\n");
}

/// Tests that the `--ellipsis` option handles multiple ellipsis sequences in one line.
///
/// Verifies that all occurrences of "..." are replaced with "…".
#[test]
fn test_cli_ellipsis_multiple_sequences() {
    let output = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("First... then second... done.\n")
        .output()
        .expect("Failed to execute mdtablefix command");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "First… then second… done.\n"
    );
}
