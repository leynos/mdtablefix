//! Integration tests for table reflow and HTML table conversion.
//!
//! Covers `reflow_table`, `convert_html_tables` and related
//! `process_stream` behaviour.

use mdtablefix::{convert_html_tables, process_stream, reflow_table};
use rstest::{fixture, rstest};

mod prelude;

#[fixture]
fn broken_table() -> Vec<String> {
    lines_vec!["| A | B |    |", "| 1 | 2 |  | 3 | 4 |"]
}

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
fn test_uniform_example_one() {
    let input = lines_vec![
        "| Logical type | PostgreSQL | SQLite notes |",
        "|--------------|-------------------------|----------------------------------------------------|",
        "| strings | `TEXT` (or `VARCHAR`) | `TEXT` - SQLite ignores the length specifier anyway |",
        "| booleans | `BOOLEAN DEFAULT FALSE` | declare as `BOOLEAN`; Diesel serialises to 0 / 1 so this is fine |",
        "| integers | `INTEGER` / `BIGINT` | ditto |",
        "| decimals | `NUMERIC` | stored as FLOAT in SQLite; Diesel `Numeric` round-trips, but beware precision |",
        "| blobs / raw | `BYTEA` | `BLOB` |",
    ];
    let output = reflow_table(&input);
    assert!(!output.is_empty());
    let widths: Vec<usize> = output[0]
        .trim_matches('|')
        .split('|')
        .map(str::len)
        .collect();
    for row in output {
        let cols: Vec<&str> = row.trim_matches('|').split('|').collect();
        for (i, col) in cols.iter().enumerate() {
            assert_eq!(col.len(), widths[i]);
        }
    }
}

#[test]
fn test_uniform_example_two() {
    let input = lines_vec![
        "| Option | How it works | When to choose it |",
        "|--------------------------------------|---------------------------------------------------------------------------------------|------------------------------------------------------------------------------|",
        "| **B. Pure-Rust migrations** | Implement `diesel::migration::Migration<DB>` in a Rust file (`up.rs` / `down.rs`) and compile with both `features = [\"postgres\", \"sqlite\"]`. The query builder emits backend-specific SQL at runtime. | You prefer the type-checked DSL and can live with slightly slower compile times. |",
        "| **C. Lowest-common-denominator SQL** | Write one `up.sql`/`down.sql` that *already* works on both engines. This demands avoiding SERIAL/IDENTITY, JSONB, `TIMESTAMPTZ`, etc. | Simple schemas, embedded use-case only, you are happy to supply integer primary keys manually. |",
        "| **D. Two separate migration trees** | Maintain `migrations/sqlite` and `migrations/postgres` directories with identical version numbers. Use `embed_migrations!(\"migrations/<backend>\")` to compile the right set. | You ship a single binary with migrations baked in. |",
    ];
    let output = reflow_table(&input);
    assert!(!output.is_empty());
    let widths: Vec<usize> = output[0]
        .trim_matches('|')
        .split('|')
        .map(str::len)
        .collect();
    for row in output {
        let cols: Vec<&str> = row.trim_matches('|').split('|').collect();
        for (i, col) in cols.iter().enumerate() {
            assert_eq!(col.len(), widths[i]);
        }
    }
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
fn test_process_stream_only_whitespace() {
    let input = lines_vec!["", "   ", "\t\t"];
    let expected = lines_vec!["", "", ""];
    assert_eq!(process_stream(&input), expected);
}

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
