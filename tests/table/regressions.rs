//! Regression tests using real data tables.

use super::*;

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
