//! Regression tests for table reflow, named for the issues they guard.
//!
//! The older suite under `tests/table/` compiles into no test target (see
//! #583), so regression cases that must run live here until it is revived.

use assert_cmd::Command;
use mdtablefix::{process_stream, reflow_table};
use rstest::rstest;

#[macro_use]
#[path = "common/mod.rs"]
mod common;

/// Regression cases for issue #582: the table pass never drops a line the
/// source wrote.
///
/// A line made only of pipes parses as a row whose cells are all empty, and
/// reflow discards such rows. In a pipe-led block with no delimiter row, which
/// is a paragraph, that deleted paragraph `|` characters; in a table, it
/// deleted an empty body row. #502 (c64ea72) exposed it by no longer taking
/// such a line for the delimiter row. The candidate is now left as written,
/// and a second pass changes nothing.
#[rstest]
#[case::lone_pipe_before_a_row(include_lines!("data/issue_582_pipe_paragraph_input.txt"))]
#[case::empty_table_row(include_lines!("data/issue_582_empty_table_row_input.txt"))]
#[case::pipes_between_rows(lines_vec!["| a | b |", "|  |  |", "| c | d |"])]
fn table_issue_582_pipe_only_line_is_kept(#[case] input: Vec<String>) {
    let once = process_stream(&input);
    assert_eq!(once, input);
    assert_eq!(process_stream(&once), once, "a second pass changes nothing");
}

/// Regression case for issue #582, the neighbouring shape: a table with no
/// pipe-only line is still reflowed.
#[test]
fn table_issue_582_block_without_pipe_only_lines_still_reflows() {
    let input = lines_vec!["| a | bb |", "| --- | --- |", "| ccc | d |"];
    let expected = lines_vec!["| a   | bb  |", "| --- | --- |", "| ccc | d   |"];
    assert_eq!(reflow_table(&input), expected);
}

/// Regression case for issue #582 through the CLI: with no flags, the
/// reproduction passes through unchanged.
#[test]
fn table_issue_582_cli_keeps_the_pipe_line() {
    let input = include_str!("data/issue_582_pipe_paragraph_input.txt");
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(input);
}
