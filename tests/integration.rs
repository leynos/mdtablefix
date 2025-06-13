use assert_cmd::Command;
use mdtablefix::{process_stream, reflow_table};
use rstest::{fixture, rstest};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[fixture]
/// Provides a sample Markdown table with broken rows for testing purposes.
///
/// The returned vector contains lines representing a table with inconsistent columns, useful for validating table reflow logic.
///
/// # Examples
///
/// ```
/// let table = broken_table();
/// assert_eq!(table[0], "| A | B |    |");
/// ```
fn broken_table() -> Vec<String> {
    vec![
        "| A | B |    |".to_string(),
        "| 1 | 2 |  | 3 | 4 |".to_string(),
    ]
}

#[fixture]
/// Returns a vector of strings representing a malformed Markdown table with inconsistent columns.
///
/// The returned table has rows with differing numbers of columns, making it invalid for standard Markdown table parsing.
///
/// # Examples
///
/// ```
/// let table = malformed_table();
/// assert_eq!(table, vec![String::from("| A | |"), String::from("| 1 | 2 | 3 |")]);
/// ```
fn malformed_table() -> Vec<String> {
    vec!["| A | |".to_string(), "| 1 | 2 | 3 |".to_string()]
}

#[fixture]
fn header_table() -> Vec<String> {
    vec![
        "| A | B |    |".to_string(),
        "| --- | --- |".to_string(),
        "| 1 | 2 |  | 3 | 4 |".to_string(),
    ]
}

#[fixture]
fn escaped_pipe_table() -> Vec<String> {
    vec![
        "| X | Y |    |".to_string(),
        "| a \\| b | 1 |  | 2 | 3 |".to_string(),
    ]
}

#[fixture]
fn indented_table() -> Vec<String> {
    vec![
        "  | I | J |    |".to_string(),
        "  | 1 | 2 |  | 3 | 4 |".to_string(),
    ]
}

#[rstest]
/// Tests that `reflow_table` correctly restructures a broken Markdown table into a well-formed table.
///
/// # Examples
///
/// ```
/// let broken = vec![String::from("| A | B |"), String::from("| 1 | 2 |"), String::from("| 3 | 4 |")];
/// let expected = vec!["| A | B |", "| 1 | 2 |", "| 3 | 4 |"];
/// assert_eq!(reflow_table(&broken), expected);
/// ```
fn test_reflow_basic(broken_table: Vec<String>) {
    let expected = vec!["| A | B |", "| 1 | 2 |", "| 3 | 4 |"];
    assert_eq!(reflow_table(&broken_table), expected);
}

#[rstest]
/// Tests that `reflow_table` returns the original input unchanged when given a malformed Markdown table.
///
/// This ensures that the function does not attempt to modify tables with inconsistent columns or structure.
fn test_reflow_malformed_returns_original(malformed_table: Vec<String>) {
    assert_eq!(reflow_table(&malformed_table), malformed_table);
}

#[rstest]
fn test_reflow_preserves_header(header_table: Vec<String>) {
    let expected = vec!["| A | B |", "| --- | --- |", "| 1 | 2 |", "| 3 | 4 |"];
    assert_eq!(reflow_table(&header_table), expected);
}

#[rstest]
fn test_reflow_handles_escaped_pipes(escaped_pipe_table: Vec<String>) {
    let expected = vec!["| X | Y |", "| a | b | 1 |", "| 2 | 3 |"];
    assert_eq!(reflow_table(&escaped_pipe_table), expected);
}

#[rstest]
fn test_reflow_preserves_indentation(indented_table: Vec<String>) {
    let expected = vec!["  | I | J |", "  | 1 | 2 |", "  | 3 | 4 |"];
    assert_eq!(reflow_table(&indented_table), expected);
}

/// Tests that `process_stream` leaves lines inside code fences unchanged.
///
/// Verifies that both backtick (```) and tilde (~~~) fenced code blocks are ignored by the table processing logic, ensuring their contents are not altered.
#[rstest]
fn test_process_stream_ignores_code_fences() {
    let lines = vec![
        "```rust".to_string(),
        "| not | a | table |".to_string(),
        "```".to_string(),
    ];
    assert_eq!(process_stream(&lines), lines);

    // Test with tilde-based code fences
    let tilde_lines = vec![
        "~~~".to_string(),
        "| not | a | table |".to_string(),
        "~~~".to_string(),
    ];
    assert_eq!(process_stream(&tilde_lines), tilde_lines);
}

#[rstest]
/// Verifies that the CLI fails when the `--in-place` flag is used without specifying a file.
///
/// This test ensures that running `mdtablefix --in-place` without a file argument results in a command failure.
///
/// # Examples
///
/// ```
/// test_cli_in_place_requires_file();
/// // The command should fail as no file is provided.
/// ```
fn test_cli_in_place_requires_file() {
    Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--in-place")
        .assert()
        .failure();
}

#[rstest]
/// Tests that the CLI processes a file containing a broken Markdown table and outputs the corrected table to stdout.
///
/// This test creates a temporary file with a malformed table, runs the `mdtablefix` binary on it, and asserts that the output is the expected fixed table.
///
/// # Examples
///
/// ```
/// let broken_table = vec![
///     "| A | B |".to_string(),
///     "| 1 | 2 |".to_string(),
///     "| 3 | 4 |".to_string(),
/// ];
/// test_cli_process_file(broken_table);
/// ```
fn test_cli_process_file(broken_table: Vec<String>) {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("sample.md");
    let mut f = File::create(&file_path).unwrap();
    for line in &broken_table {
        writeln!(f, "{}", line).unwrap();
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
