//! Tests for `convert_html_tables`.
//!
//! This module provides comprehensive unit tests for HTML table to markdown conversion,
//! covering standard tables, edge cases with colspan attributes, malformed HTML,
//! and various header scenarios.

use mdtablefix::convert_html_tables;

#[macro_use]
#[path = "../prelude/mod.rs"]
mod prelude;
use prelude::*;

use super::fixtures::*;

#[rstest(
    input,
    expected,
    case::basic(html_table(), lines_vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"]),
    case::with_attrs(html_table_with_attrs(), lines_vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"]),
    case::uppercase(html_table_uppercase(), lines_vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"]),
)]
fn test_convert_html_table_standard(input: Vec<String>, expected: Vec<String>) {
    assert_eq!(convert_html_tables(&input), expected);
}

#[rstest(
    input,
    expected,
    case::colspan(html_table_with_colspan(), lines_vec!["| A |", "| --- |", "| 1 | 2 |"]),
    case::inconsistent(html_table_inconsistent_first_row(), lines_vec!["| A |", "| --- |", "| 1 | 2 |"]),
)]
fn test_convert_html_table_reduced(input: Vec<String>, expected: Vec<String>) {
    assert_eq!(convert_html_tables(&input), expected);
}

#[test]
fn test_convert_html_table_no_header() {
    let expected = lines_vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"];
    assert_eq!(convert_html_tables(&html_table_no_header()), expected);
}

#[test]
fn test_convert_html_table_empty_row() {
    let expected = lines_vec!["| 1 | 2 |", "| --- | --- |"];
    assert_eq!(convert_html_tables(&html_table_empty_row()), expected);
}

#[test]
fn test_convert_html_table_whitespace_header() {
    let expected = lines_vec!["| --- | --- |", "| --- | --- |", "| 1   | 2   |"];
    assert_eq!(
        convert_html_tables(&html_table_whitespace_header()),
        expected
    );
}

#[test]
fn test_convert_html_table_empty() {
    assert!(convert_html_tables(&html_table_empty()).is_empty());
}

#[test]
fn test_convert_html_table_unclosed_returns_original() {
    let html = html_table_unclosed();
    assert_eq!(convert_html_tables(&html), html);
}

#[test]
fn test_convert_html_table_bold_header() {
    let input: Vec<String> = include_lines!("data/bold_header_input.txt");
    let expected: Vec<String> = include_lines!("data/bold_header_expected.txt");
    assert_eq!(convert_html_tables(&input), expected);
}
