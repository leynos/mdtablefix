//! Integration tests for table reflow and HTML table conversion.
//!
//! This module provides comprehensive test coverage for the table processing
//! functionality in `mdtablefix`, including Markdown table reflowing and
//! HTML-to-Markdown conversion.
//!
//! The module is organised into focused submodules:
//! - `reflow`: Tests for `reflow_table` covering basic reflow, malformed tables,
//!   header preservation, escaped pipes and indentation.
//! - `process_stream_tests`: Tests for `process_stream` verifying normalisation
//!   of various HTML table variants and handling of multiple tables.
//! - `uniform`: Regression tests ensuring uniform column widths after reflowing.
//! - `convert_html`: Parameterised tests for HTML table conversion edge cases.
//! - `regressions`: Real-world data validation tests.
//!
//! Each test uses fixtures defined in this module to ensure consistent test data
//! across different scenarios whilst avoiding duplication.

use mdtablefix::{convert_html_tables, process_stream, reflow_table};

#[macro_use]
mod prelude;
use prelude::*;

#[fixture]
fn malformed_table() -> Vec<String> {
    lines_vec!["| A | |", "| 1 | 2 | 3 |"]
}

#[fixture]
fn header_table() -> Vec<String> {
    lines_vec!["| A | B |    |", "| --- | --- |", "| 1 | 2 |  | 3 | 4 |"]
}

#[fixture]
fn escaped_pipe_table() -> Vec<String> {
    lines_vec!["| X | Y |    |", "| a \\| b | 1 |  | 2 | 3 |"]
}

#[fixture]
fn indented_table() -> Vec<String> {
    lines_vec!["  | I | J |    |", "  | 1 | 2 |  | 3 | 4 |"]
}

#[fixture]
fn html_table() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
fn html_table_with_attrs() -> Vec<String> {
    lines_vec![
        "<table class=\"x\">",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
fn html_table_with_colspan() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr><th colspan=\"2\">A</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
fn html_table_no_header() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr><td>A</td><td>B</td></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
fn html_table_empty_row() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
fn html_table_whitespace_header() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr><td>  </td><td>  </td></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
fn html_table_inconsistent_first_row() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr><td>A</td></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
fn html_table_empty() -> Vec<String> {
    lines_vec!["<table></table>"]
}

#[fixture]
fn html_table_unclosed() -> Vec<String> {
    lines_vec!["<table>", "<tr><td>1</td></tr>"]
}

#[fixture]
fn html_table_uppercase() -> Vec<String> {
    lines_vec![
        "<TABLE>",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</TABLE>",
    ]
}

#[fixture]
fn html_table_mixed_case() -> Vec<String> {
    lines_vec![
        "<TaBlE>",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</TaBlE>",
    ]
}

#[fixture]
fn multiple_tables() -> Vec<String> {
    lines_vec!["| A | B |", "| 1 | 22 |", "", "| X | Y |", "| 3 | 4 |"]
}

mod reflow;
mod process_stream_tests;
mod uniform;
mod convert_html;
mod regressions;
