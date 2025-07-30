//! Additional table processing regression tests using external data files.

use mdtablefix::{convert_html_tables, process_stream, reflow_table};

#[macro_use]
mod prelude;
#[path = "table_fixtures.rs"]
mod table_fixtures;
use table_fixtures::*;

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

#[test]
fn test_logical_type_table_output_matches() {
    let input: Vec<String> = include_lines!("data/logical_type_input.txt");
    let expected: Vec<String> = include_lines!("data/logical_type_expected.txt");
    assert_eq!(reflow_table(&input), expected);
}

#[test]
fn test_option_table_output_matches() {
    let input: Vec<String> = include_lines!("data/option_table_input.txt");
    let expected: Vec<String> = include_lines!("data/option_table_expected.txt");
    assert_eq!(reflow_table(&input), expected);
}

#[test]
fn test_month_seconds_table_output_matches() {
    let input: Vec<String> = include_lines!("data/month_seconds_input.txt");
    let expected: Vec<String> = include_lines!("data/month_seconds_expected.txt");
    assert_eq!(reflow_table(&input), expected);
}

#[test]
fn test_offset_table_output_matches() {
    let input: Vec<String> = include_lines!("data/offset_table_input.txt");
    let expected: Vec<String> = include_lines!("data/offset_table_expected.txt");
    assert_eq!(reflow_table(&input), expected);
}

#[test]
fn test_process_stream_logical_type_table() {
    let input: Vec<String> = include_lines!("data/logical_type_input.txt");
    let expected: Vec<String> = include_lines!("data/logical_type_expected.txt");
    assert_eq!(process_stream(&input), expected);
}

#[test]
fn test_process_stream_option_table() {
    let input: Vec<String> = include_lines!("data/option_table_input.txt");
    let expected: Vec<String> = include_lines!("data/option_table_expected.txt");
    assert_eq!(process_stream(&input), expected);
}

#[test]
fn test_regression_complex_table() {
    let input: Vec<String> = include_lines!("data/regression_table_input.txt");
    let expected: Vec<String> = include_lines!("data/regression_table_expected.txt");
    assert_eq!(process_stream(&input), expected);
}
