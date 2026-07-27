//! Tests for `process_stream` table behaviour.

use rstest::rstest;
use super::*;

#[rstest(
    table,
    case::basic(html_table()),
    case::attrs(html_table_with_attrs()),
    case::uppercase(html_table_uppercase()),
    case::mixed_case(html_table_mixed_case())
)]
fn test_process_stream_html_variants(table: Vec<String>) {
    let expected = lines_vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"];
    assert_eq!(process_stream(&table), expected);
}

#[rstest]
fn test_process_stream_multiple_tables(multiple_tables: Vec<String>) {
    let expected = lines_vec!["| A | B  |", "| 1 | 22 |", "", "| X | Y |", "| 3 | 4 |"];
    assert_eq!(process_stream(&multiple_tables), expected);
}

#[rstest]
fn test_process_stream_ignores_code_fences() {
    let lines = lines_vec!["```rust", "| not | a | table |", "```"];
    assert_eq!(process_stream(&lines), lines);

    let tilde_lines = lines_vec!["~~~", "| not | a | table |", "~~~"];
    assert_eq!(process_stream(&tilde_lines), tilde_lines);
}

#[rstest]
fn test_process_stream_ignores_indented_fences() {
    let lines = lines_vec!(
        "   ```javascript",
        "   socket.onmessage = function(event) {",
        "       const message = JSON.parse(event.data);",
        "       switch(message.type) {",
        "           case \"serverNewMessage\":",
        "               // Display message.payload.user and message.payload.text",
        "               break;",
        "           case \"serverUserJoined\":",
        "               // Update user list with message.payload.user",
        "               break;",
        "           // Handle other message types...",
        "       }",
        "   };",
        "",
        "   ```",
    );
    assert_eq!(process_stream(&lines), lines);
}

#[test]
fn test_non_table_lines_unchanged() {
    let input = lines_vec![
        "# Title",
        "",
        "Para text.",
        "",
        "| a | b |",
        "| 1 | 22 |",
        "",
        "* bullet",
        "",
    ];
    let output = process_stream(&input);
    let expected = lines_vec![
        "# Title",
        "",
        "Para text.",
        "",
        "| a | b  |",
        "| 1 | 22 |",
        "",
        "* bullet",
        "",
    ];
    assert_eq!(output, expected);
}

#[test]
fn test_process_stream_reflows_table_before_numeric_paragraph() {
    let input = lines_vec![
        "| a | b |",
        "| 1 | 22 |",
        "2024 revenue climbed 10%",
    ];
    let expected = lines_vec![
        "| a | b  |",
        "| 1 | 22 |",
        "2024 revenue climbed 10%",
    ];
    assert_eq!(process_stream(&input), expected);
}

#[test]
fn flushes_table_before_plain_paragraph_no_blank() {
    let input = vec![
        "| a | b |".to_string(),
        "|---|---|".to_string(),
        "AWS revenue rose".to_string(),
    ];
    let out = crate::process::process_stream(&input);
    assert!(out.iter().any(|l| l.contains("| a | b |")));
    assert!(out.iter().any(|l| l == "AWS revenue rose"));
}

#[test]
fn test_process_stream_reflows_table_before_heading() {
    let input = lines_vec![
        "| a | b |",
        "| 1 | 22 |",
        "# Heading",
    ];
    let expected = lines_vec![
        "| a | b  |",
        "| 1 | 22 |",
        "# Heading",
    ];
    assert_eq!(process_stream(&input), expected);
}


#[test]
fn test_process_stream_only_whitespace() {
    let input = lines_vec!["", "   ", "\t\t"];
    let expected = lines_vec!["", "", ""];
    assert_eq!(process_stream(&input), expected);
}
