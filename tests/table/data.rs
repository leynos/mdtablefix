//! Tests using external table data files.

use mdtablefix::{process_stream, reflow_table};

#[macro_use]
#[path = "../prelude/mod.rs"]
mod prelude;
use prelude::*;

#[rstest(
    input_file,
    expected_file,
    case::logical_type("data/logical_type_input.txt", "data/logical_type_expected.txt"),
    case::option_table("data/option_table_input.txt", "data/option_table_expected.txt"),
    case::month_seconds("data/month_seconds_input.txt", "data/month_seconds_expected.txt"),
    case::offset_table("data/offset_table_input.txt", "data/offset_table_expected.txt"),
)]
fn test_reflow_table_data_files(input_file: &str, expected_file: &str) {
    let input: Vec<String> = include_lines!(input_file);
    let expected: Vec<String> = include_lines!(expected_file);
    assert_eq!(reflow_table(&input), expected);
}

#[rstest(
    input_file,
    expected_file,
    case::logical_type("data/logical_type_input.txt", "data/logical_type_expected.txt"),
    case::option_table("data/option_table_input.txt", "data/option_table_expected.txt"),
    case::regression("data/regression_table_input.txt", "data/regression_table_expected.txt"),
)]
fn test_process_stream_data_files(input_file: &str, expected_file: &str) {
    let input: Vec<String> = include_lines!(input_file);
    let expected: Vec<String> = include_lines!(expected_file);
    assert_eq!(process_stream(&input), expected);
}
