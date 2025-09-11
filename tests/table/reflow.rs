//! Tests for the `reflow_table` function.

use rstest::rstest;
use super::*;

#[rstest]
fn test_reflow_basic(broken_table: Vec<String>) {
    let expected = lines_vec!["| A | B |", "| 1 | 2 |", "| 3 | 4 |"];
    assert_eq!(reflow_table(&broken_table), expected);
}

#[rstest]
fn test_reflow_malformed_returns_original(malformed_table: Vec<String>) {
    assert_eq!(reflow_table(&malformed_table), malformed_table);
}

#[rstest]
fn test_reflow_preserves_header(header_table: Vec<String>) {
    let expected = lines_vec!["| A | B |", "| --- | --- |", "| 1 | 2 |", "| 3 | 4 |"];
    assert_eq!(reflow_table(&header_table), expected);
}

#[rstest]
fn test_reflow_handles_escaped_pipes(escaped_pipe_table: Vec<String>) {
    let expected = lines_vec!["| X     | Y |", "| a | b | 1 |", "| 2     | 3 |"];
    assert_eq!(reflow_table(&escaped_pipe_table), expected);
}

#[rstest]
fn test_reflow_preserves_indentation(indented_table: Vec<String>) {
    let expected = lines_vec!["  | I | J |", "  | 1 | 2 |", "  | 3 | 4 |"];
    assert_eq!(reflow_table(&indented_table), expected);
}
