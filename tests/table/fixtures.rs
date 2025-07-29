//! Fixtures providing sample tables for tests.

#[macro_use]
#[path = "../prelude/mod.rs"]
mod prelude;
use prelude::*;

#[fixture]
pub fn malformed_table() -> Vec<String> {
    let lines = lines_vec!["| A | |", "| 1 | 2 | 3 |"];
    lines
}

#[fixture]
pub fn header_table() -> Vec<String> {
    lines_vec!["| A | B |    |", "| --- | --- |", "| 1 | 2 |  | 3 | 4 |"]
}

#[fixture]
pub fn escaped_pipe_table() -> Vec<String> {
    lines_vec!["| X | Y |    |", "| a \\| b | 1 |  | 2 | 3 |"]
}

#[fixture]
pub fn indented_table() -> Vec<String> {
    let lines = lines_vec!["  | I | J |    |", "  | 1 | 2 |  | 3 | 4 |"];
    lines
}

#[fixture]
pub fn html_table() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
pub fn html_table_with_attrs() -> Vec<String> {
    lines_vec![
        "<table class=\"x\">",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
pub fn html_table_with_colspan() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr><th colspan=\"2\">A</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
pub fn html_table_no_header() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr><td>A</td><td>B</td></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
pub fn html_table_empty_row() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
pub fn html_table_whitespace_header() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr><td>  </td><td>  </td></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
pub fn html_table_inconsistent_first_row() -> Vec<String> {
    lines_vec![
        "<table>",
        "<tr><td>A</td></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    ]
}

#[fixture]
pub fn html_table_empty() -> Vec<String> {
    let lines = lines_vec!["<table></table>"];
    lines
}

#[fixture]
pub fn html_table_unclosed() -> Vec<String> {
    let lines = lines_vec!["<table>", "<tr><td>1</td></tr>"];
    lines
}

#[fixture]
pub fn html_table_uppercase() -> Vec<String> {
    lines_vec![
        "<TABLE>",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</TABLE>",
    ]
}

#[fixture]
pub fn html_table_mixed_case() -> Vec<String> {
    lines_vec![
        "<TaBlE>",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</TaBlE>",
    ]
}

#[fixture]
pub fn multiple_tables() -> Vec<String> {
    lines_vec!["| A | B |", "| 1 | 22 |", "", "| X | Y |", "| 3 | 4 |"]
}
