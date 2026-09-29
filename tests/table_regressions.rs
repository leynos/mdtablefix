//! Regression tests for table reflow, named for the issues they guard.
//!
//! The older suite under `tests/table/` compiles into no test target (see
//! #583), so regression cases that must run live here until it is revived.

use assert_cmd::Command;
use mdtablefix::{convert_html_tables, process_stream, reflow_table};
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

/// Regression case for issue #582, the HTML path: an empty `<tr>` becomes a
/// pipe-only row, and the generated table is still aligned with that row kept.
///
/// Source tables with a pipe-only line are left as written, but the HTML
/// converter builds its own lines, so it reflows the rest and puts each empty
/// row back padded to the delimiter row's geometry.
#[test]
fn table_issue_582_html_table_keeps_its_empty_row_aligned() {
    let input = lines_vec![
        "<table>",
        "<tr><th>Name</th><th>Value</th></tr>",
        "<tr><td>alpha</td><td>1</td></tr>",
        "<tr><td></td><td></td></tr>",
        "<tr><td>b</td><td>22</td></tr>",
        "</table>",
    ];
    let expected = lines_vec![
        "| Name  | Value |",
        "| ----- | ----- |",
        "| alpha | 1     |",
        "|       |       |",
        "| b     | 22    |",
    ];
    let once = convert_html_tables(&input);
    assert_eq!(once, expected);
    assert_eq!(process_stream(&once), once, "a second pass changes nothing");
}
