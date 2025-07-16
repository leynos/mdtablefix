use std::{fs::File, io::Write};

use assert_cmd::Command;
use mdtablefix::{
    THEMATIC_BREAK_LEN,
    convert_html_tables,
    format_breaks,
    process_stream,
    reflow_table,
    renumber_lists,
};
use rstest::{fixture, rstest};
use tempfile::tempdir;

#[macro_use]
mod common;

#[fixture]
/// Provides a sample Markdown table with broken rows for testing purposes.
///
/// The returned vector contains lines representing a table with inconsistent columns, useful for
/// validating table reflow logic.
///
/// # Examples
///
/// ```
/// let table = broken_table();
/// assert_eq!(table[0], "| A | B |    |");
/// ```
fn broken_table() -> Vec<String> {
    let lines = lines_vec!("| A | B |    |", "| 1 | 2 |  | 3 | 4 |",);
    lines
}

#[fixture]
/// Returns a vector of strings representing a malformed Markdown table with inconsistent columns.
///
/// The returned table has rows with differing numbers of columns, making it invalid for standard
/// Markdown table parsing.
///
/// # Examples
///
/// ```
/// let table = malformed_table();
/// assert_eq!(
///     table,
///     vec![String::from("| A | |"), String::from("| 1 | 2 | 3 |")]
/// );
/// ```
fn malformed_table() -> Vec<String> {
    let lines = lines_vec!("| A | |", "| 1 | 2 | 3 |");
    lines
}

#[fixture]
fn header_table() -> Vec<String> {
    lines_vec!("| A | B |    |", "| --- | --- |", "| 1 | 2 |  | 3 | 4 |",)
}

#[fixture]
fn escaped_pipe_table() -> Vec<String> {
    lines_vec!("| X | Y |    |", "| a \\| b | 1 |  | 2 | 3 |",)
}

#[fixture]
fn indented_table() -> Vec<String> {
    let lines = lines_vec!("  | I | J |    |", "  | 1 | 2 |  | 3 | 4 |",);
    lines
}

#[fixture]
fn html_table() -> Vec<String> {
    lines_vec!(
        "<table>",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    )
}

#[fixture]
fn html_table_with_attrs() -> Vec<String> {
    lines_vec!(
        "<table class=\"x\">",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    )
}

#[fixture]
fn html_table_with_colspan() -> Vec<String> {
    lines_vec!(
        "<table>",
        "<tr><th colspan=\"2\">A</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    )
}

#[fixture]
fn html_table_no_header() -> Vec<String> {
    lines_vec!(
        "<table>",
        "<tr><td>A</td><td>B</td></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    )
}

#[fixture]
fn html_table_empty_row() -> Vec<String> {
    lines_vec!(
        "<table>",
        "<tr></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    )
}

#[fixture]
fn html_table_whitespace_header() -> Vec<String> {
    lines_vec!(
        "<table>",
        "<tr><td>  </td><td>  </td></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    )
}

#[fixture]
fn html_table_inconsistent_first_row() -> Vec<String> {
    lines_vec!(
        "<table>",
        "<tr><td>A</td></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</table>",
    )
}

#[fixture]
fn html_table_empty() -> Vec<String> {
    let lines = lines_vec!("<table></table>");
    lines
}

#[fixture]
fn html_table_unclosed() -> Vec<String> {
    let lines = lines_vec!("<table>", "<tr><td>1</td></tr>");
    lines
}

#[fixture]
fn html_table_uppercase() -> Vec<String> {
    lines_vec!(
        "<TABLE>",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</TABLE>",
    )
}

#[fixture]
fn html_table_mixed_case() -> Vec<String> {
    lines_vec!(
        "<TaBlE>",
        "<tr><th>A</th><th>B</th></tr>",
        "<tr><td>1</td><td>2</td></tr>",
        "</TaBlE>",
    )
}

#[fixture]
fn multiple_tables() -> Vec<String> {
    lines_vec!("| A | B |", "| 1 | 22 |", "", "| X | Y |", "| 3 | 4 |",)
}

#[rstest]
/// Tests that `reflow_table` correctly restructures a broken Markdown table into a well-formed
/// table.
///
/// # Examples
///
/// ```
/// let broken = vec![
///     String::from("| A | B |"),
///     String::from("| 1 | 2 |"),
///     String::from("| 3 | 4 |"),
/// ];
/// let expected = vec!["| A | B |", "| 1 | 2 |", "| 3 | 4 |"];
/// assert_eq!(reflow_table(&broken), expected);
/// ```
fn test_reflow_basic(broken_table: Vec<String>) {
    let expected = vec!["| A | B |", "| 1 | 2 |", "| 3 | 4 |"];
    assert_eq!(reflow_table(&broken_table), expected);
}

#[rstest]
/// Tests that `reflow_table` returns the original input unchanged when given a malformed Markdown
/// table.
///
/// This ensures that the function does not attempt to modify tables with inconsistent columns or
/// structure.
fn test_reflow_malformed_returns_original(malformed_table: Vec<String>) {
    assert_eq!(reflow_table(&malformed_table), malformed_table);
}

#[rstest]
fn test_reflow_preserves_header(header_table: Vec<String>) {
    let expected = vec!["| A | B |", "| --- | --- |", "| 1 | 2 |", "| 3 | 4 |"];
    assert_eq!(reflow_table(&header_table), expected);
}

#[rstest]
fn test_reflow_handles_escaped_pipes(escaped_pipe_table: Vec<String>) {
    // The fixture contains a header row followed by a row with an escaped
    // pipe sequence (`a \| b`). After reflow the escaped pipe becomes a literal
    // `|` inside the first data cell, so the table has three columns and the
    // header row is padded to match.
    let expected = vec!["| X     | Y |", "| a | b | 1 |", "| 2     | 3 |"];
    assert_eq!(reflow_table(&escaped_pipe_table), expected);
}

#[rstest]
fn test_reflow_preserves_indentation(indented_table: Vec<String>) {
    let expected = vec!["  | I | J |", "  | 1 | 2 |", "  | 3 | 4 |"];
    assert_eq!(reflow_table(&indented_table), expected);
}

#[rstest]
fn test_process_stream_html_table(html_table: Vec<String>) {
    let expected = vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"];
    assert_eq!(process_stream(&html_table), expected);
}

#[rstest]
fn test_process_stream_html_table_with_attrs(html_table_with_attrs: Vec<String>) {
    let expected = vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"];
    assert_eq!(process_stream(&html_table_with_attrs), expected);
}

#[rstest]
fn test_process_stream_html_table_uppercase(html_table_uppercase: Vec<String>) {
    let expected = vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"];
    assert_eq!(process_stream(&html_table_uppercase), expected);
}

#[rstest]
fn test_process_stream_html_table_mixed_case(html_table_mixed_case: Vec<String>) {
    let expected = vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"];
    assert_eq!(process_stream(&html_table_mixed_case), expected);
}

#[rstest]
fn test_process_stream_multiple_tables(multiple_tables: Vec<String>) {
    let expected = lines_vec!(
        "| A | B  |",
        "| 1 | 22 |",
        String::new(),
        "| X | Y |",
        "| 3 | 4 |",
    );
    assert_eq!(process_stream(&multiple_tables), expected);
}

/// Tests that `process_stream` leaves lines inside code fences unchanged.
///
/// Verifies that both backtick (```) and tilde (~~~) fenced code blocks are ignored by the table
/// processing logic, ensuring their contents are not altered.
#[rstest]
fn test_process_stream_ignores_code_fences() {
    let lines = lines_vec!("```rust", "| not | a | table |", "```");
    assert_eq!(process_stream(&lines), lines);

    // Test with tilde-based code fences
    let tilde_lines = lines_vec!("~~~", "| not | a | table |", "~~~");
    assert_eq!(process_stream(&tilde_lines), tilde_lines);
}

#[rstest]
/// Verifies that the CLI fails when the `--in-place` flag is used without specifying a file.
///
/// This test ensures that running `mdtablefix --in-place` without a file argument results in a
/// command failure.
///
/// # Examples
///
/// ```
/// test_cli_in_place_requires_file();
/// // The command should fail as no file is provided.
/// ```
fn test_cli_in_place_requires_file() {
    Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--in-place")
        .assert()
        .failure();
}

#[rstest]
/// Tests that the CLI processes a file containing a broken Markdown table and outputs the corrected
/// table to stdout.
///
/// This test creates a temporary file with a malformed table, runs the `mdtablefix` binary on it,
/// and asserts that the output is the expected fixed table.
///
/// # Examples
///
/// ```
/// let broken_table = vec![
///     "| A | B |".to_string(),
///     "| 1 | 2 |".to_string(),
///     "| 3 | 4 |".to_string(),
/// ];
/// test_cli_process_file(broken_table);
/// ```
fn test_cli_process_file(broken_table: Vec<String>) {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("sample.md");
    let mut f = File::create(&file_path).unwrap();
    for line in &broken_table {
        writeln!(f, "{line}").unwrap();
    }
    f.flush().unwrap();
    drop(f);
    Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg(&file_path)
        .assert()
        .success()
        .stdout("| A | B |\n| 1 | 2 |\n| 3 | 4 |\n");
}

#[test]
fn test_cli_wrap_option() {
    let input = "This line is deliberately made much longer than eighty columns so that the \
                 wrapping algorithm is forced to insert a soft line-break somewhere in the middle \
                 of the paragraph when the --wrap flag is supplied.";
    let output = Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--wrap")
        .write_stdin(format!("{input}\n"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.lines().count() > 1,
        "expected wrapped output on multiple lines"
    );
    assert!(text.lines().all(|l| l.len() <= 80));
}

#[test]
fn test_uniform_example_one() {
    let input = vec![
        "| Logical type | PostgreSQL | SQLite notes |".to_string(),
        "|--------------|-------------------------|---------------------------------------------------------------------------------|".to_string(),
        "| strings | `TEXT` (or `VARCHAR`) | `TEXT` - SQLite ignores the length specifier anyway |".to_string(),
        "| booleans | `BOOLEAN DEFAULT FALSE` | declare as `BOOLEAN`; Diesel serialises to 0 / 1 so this is fine |".to_string(),
        "| integers | `INTEGER` / `BIGINT` | ditto |".to_string(),
        "| decimals | `NUMERIC` | stored as FLOAT in SQLite; Diesel `Numeric` round-trips, but beware precision |".to_string(),
        "| blobs / raw | `BYTEA` | `BLOB` |".to_string(),
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
    let input = vec![
        "| Option | How it works | When to choose it |".to_string(),
        "|--------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------|".to_string(),
        "| **B. Pure-Rust migrations** | Implement `diesel::migration::Migration<DB>` in a Rust file (`up.rs` / `down.rs`) and compile with both `features = [\"postgres\", \"sqlite\"]`. The query builder emits backend-specific SQL at runtime. | You prefer the type-checked DSL and can live with slightly slower compile times. |".to_string(),
        "| **C. Lowest-common-denominator SQL** | Write one `up.sql`/`down.sql` that *already* works on both engines. This demands avoiding SERIAL/IDENTITY, JSONB, `TIMESTAMPTZ`, etc. | Simple schemas, embedded use-case only, you are happy to supply integer primary keys manually. |".to_string(),
        "| **D. Two separate migration trees** | Maintain `migrations/sqlite` and `migrations/postgres` directories with identical version numbers. Use `embed_migrations!(\"migrations/<backend>\")` to compile the right set. | You ship a single binary with migrations baked in. |".to_string(),
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
    let input = vec![
        "# Title".to_string(),
        String::new(),
        "Para text.".to_string(),
        String::new(),
        "| a | b |".to_string(),
        "| 1 | 22 |".to_string(),
        String::new(),
        "* bullet".to_string(),
        String::new(),
    ];
    let output = process_stream(&input);
    let expected = vec![
        "# Title".to_string(),
        String::new(),
        "Para text.".to_string(),
        String::new(),
        "| a | b  |".to_string(),
        "| 1 | 22 |".to_string(),
        String::new(),
        "* bullet".to_string(),
        String::new(),
    ];
    assert_eq!(output, expected);
}

#[test]
fn test_convert_html_table_basic() {
    let html_table = vec![
        "<table>".to_string(),
        "<tr><th>A</th><th>B</th></tr>".to_string(),
        "<tr><td>1</td><td>2</td></tr>".to_string(),
        "</table>".to_string(),
    ];
    let expected = vec![
        "| A | B |".to_string(),
        "| --- | --- |".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    assert_eq!(convert_html_tables(&html_table), expected);
}

#[rstest]
#[case("```")]
#[case("~~~")]
#[case("```rust")]
fn test_convert_html_table_in_text_and_code(#[case] fence: &str) {
    let lines = vec![
        "Intro".to_string(),
        "<table>".to_string(),
        "<tr><th>A</th><th>B</th></tr>".to_string(),
        "<tr><td>1</td><td>2</td></tr>".to_string(),
        "</table>".to_string(),
        fence.to_string(),
        "<table><tr><td>x</td></tr></table>".to_string(),
        fence.to_string(),
        "Outro".to_string(),
    ];
    let expected = vec![
        "Intro".to_string(),
        "| A | B |".to_string(),
        "| --- | --- |".to_string(),
        "| 1 | 2 |".to_string(),
        fence.to_string(),
        "<table><tr><td>x</td></tr></table>".to_string(),
        fence.to_string(),
        "Outro".to_string(),
    ];
    assert_eq!(convert_html_tables(&lines), expected);
}

#[test]
fn test_convert_html_table_with_attrs_basic() {
    let expected = vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"];
    assert_eq!(convert_html_tables(&html_table_with_attrs()), expected);
}

#[test]
fn test_convert_html_table_uppercase() {
    let expected = vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"];
    assert_eq!(convert_html_tables(&html_table_uppercase()), expected);
}

#[test]
fn test_convert_html_table_with_colspan() {
    let expected = vec!["| A |", "| --- |", "| 1 | 2 |"];
    assert_eq!(convert_html_tables(&html_table_with_colspan()), expected);
}

#[test]
fn test_convert_html_table_no_header() {
    let expected = vec!["| A | B |", "| --- | --- |", "| 1 | 2 |"];
    assert_eq!(convert_html_tables(&html_table_no_header()), expected);
}

#[test]
fn test_convert_html_table_empty_row() {
    let expected = vec!["| 1 | 2 |", "| --- | --- |"];
    assert_eq!(convert_html_tables(&html_table_empty_row()), expected);
}

#[test]
fn test_convert_html_table_whitespace_header() {
    let expected = vec!["| --- | --- |", "| --- | --- |", "| 1   | 2   |"];
    assert_eq!(
        convert_html_tables(&html_table_whitespace_header()),
        expected
    );
}

#[test]
fn test_convert_html_table_inconsistent_first_row() {
    let expected = vec!["| A |", "| --- |", "| 1 | 2 |"];
    assert_eq!(
        convert_html_tables(&html_table_inconsistent_first_row()),
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
    let input: Vec<String> = include_str!("data/bold_header_input.txt")
        .lines()
        .map(str::to_string)
        .collect();
    let expected: Vec<String> = include_str!("data/bold_header_expected.txt")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(convert_html_tables(&input), expected);
}

#[test]
fn test_logical_type_table_output_matches() {
    let input: Vec<String> = include_str!("data/logical_type_input.txt")
        .lines()
        .map(str::to_string)
        .collect();
    let expected: Vec<String> = include_str!("data/logical_type_expected.txt")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(reflow_table(&input), expected);
}

#[test]
/// Verifies that reflowing the option table input produces the expected output.
///
/// Loads the input and expected output from external files and asserts that the
/// `reflow_table` function transforms the input table to match the expected result.
fn test_option_table_output_matches() {
    let input: Vec<String> = include_str!("data/option_table_input.txt")
        .lines()
        .map(str::to_string)
        .collect();
    let expected: Vec<String> = include_str!("data/option_table_expected.txt")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(reflow_table(&input), expected);
}

#[test]
fn test_month_seconds_table_output_matches() {
    let input: Vec<String> = include_str!("data/month_seconds_input.txt")
        .lines()
        .map(str::to_string)
        .collect();
    let expected: Vec<String> = include_str!("data/month_seconds_expected.txt")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(reflow_table(&input), expected);
}

#[test]
fn test_offset_table_output_matches() {
    let input: Vec<String> = include_str!("data/offset_table_input.txt")
        .lines()
        .map(str::to_string)
        .collect();
    let expected: Vec<String> = include_str!("data/offset_table_expected.txt")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(reflow_table(&input), expected);
}

#[test]
/// Tests that `process_stream` correctly processes a complex Markdown table representing logical
/// types by comparing its output to expected results loaded from a file.
fn test_process_stream_logical_type_table() {
    let input: Vec<String> = include_str!("data/logical_type_input.txt")
        .lines()
        .map(str::to_string)
        .collect();
    let expected: Vec<String> = include_str!("data/logical_type_expected.txt")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(process_stream(&input), expected);
}

#[test]
/// Tests that `process_stream` correctly processes a Markdown table with options, producing the
/// expected output.
///
/// Loads input and expected output from test data files, runs `process_stream` on the input, and
/// asserts equality.
///
/// # Examples
///
/// ```
/// test_process_stream_option_table(); 
/// ```
fn test_process_stream_option_table() {
    let input: Vec<String> = include_str!("data/option_table_input.txt")
        .lines()
        .map(str::to_string)
        .collect();
    let expected: Vec<String> = include_str!("data/option_table_expected.txt")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(process_stream(&input), expected);
}

#[test]
/// Tests that long paragraphs are wrapped at 80 columns by `process_stream`.
///
/// Ensures that a single long paragraph is split into multiple lines, each not exceeding 80
/// characters.
fn test_wrap_paragraph() {
    let input = vec![
        "This is a very long paragraph that should be wrapped at eighty columns so it needs to \
         contain enough words to exceed that limit."
            .to_string(),
    ];
    let output = process_stream(&input);
    assert!(output.len() > 1);
    assert!(output.iter().all(|l| l.len() <= 80));
}

#[test]
fn test_wrap_list_item() {
    let input = vec![
        r"- This bullet item is exceptionally long and must be wrapped to keep prefix formatting intact."
            .to_string(),
    ];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "- ", 2);
}

#[rstest]
#[case("- ", 3)]
#[case("1. ", 3)]
#[case("10. ", 3)]
#[case("100. ", 3)]
fn test_wrap_list_items_with_inline_code(#[case] prefix: &str, #[case] expected: usize) {
    let input = vec![format!(
        "{prefix}`script`: A multi-line script declared with the YAML `|` block style. The entire \
         block is passed to an interpreter. If the first line begins with `#!`, Netsuke executes \
         the script verbatim, respecting the shebang."
    )];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, prefix, expected);
}

#[test]
fn test_wrap_preserves_inline_code_spans() {
    let input = vec![
        "- `script`: A multi-line script declared with the YAML `|` block style. The entire block \
         is passed to an interpreter. If the first line begins with `#!`, Netsuke executes the \
         script verbatim, respecting the shebang."
            .to_string(),
    ];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "- ", 3);
}

#[test]
fn test_wrap_multi_backtick_code() {
    let input = vec![
        "- ``cmd`` executes ```echo``` output with ``json`` format and prints results to the \
         console"
            .to_string(),
    ];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "- ", 2);
}

#[test]
fn test_wrap_multiple_inline_code_spans() {
    let input = vec![
        "- Use `foo` and `bar` inside ``baz`` for testing with additional commentary to exceed \
         wrapping width"
            .to_string(),
    ];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "- ", 2);
}

#[test]
fn test_wrap_footnote_multiline() {
    let input = vec![
        concat!(
            "[^note]: This footnote is sufficiently long to require wrapping ",
            "across multiple lines so we can verify indentation."
        )
        .to_string(),
    ];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "[^note]: ", 2);
}

#[test]
fn test_wrap_footnote_with_inline_code() {
    let input = vec![
        concat!(
            "  [^code_note]: A footnote containing inline `code` that should wrap ",
            "across multiple lines without breaking the span."
        )
        .to_string(),
    ];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "  [^code_note]: ", 2);
}

#[test]
/// Verifies that short list items are not wrapped or altered by the stream processing logic.
///
/// Ensures that a single-line bullet list item remains unchanged after processing.
///
/// # Examples
///
/// ```
/// let input = vec!["- short item".to_string()];
/// let output = process_stream(&input);
/// assert_eq!(output, input);
/// ```
fn test_wrap_short_list_item() {
    let input = vec!["- short item".to_string()];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

#[test]
fn test_wrap_blockquote() {
    let input = vec![
        "> **Deprecated**: A :class:`WebSocketRouter` and its `add_route` API should be used to \
         instantiate resources."
            .to_string(),
    ];
    let output = process_stream(&input);
    assert_eq!(
        output,
        vec![
            "> **Deprecated**: A :class:`WebSocketRouter` and its `add_route` API should be"
                .to_string(),
            "> used to instantiate resources.".to_string(),
        ]
    );
}

#[test]
/// Tests that lines with hard line breaks (trailing spaces) are preserved after processing.
///
/// Ensures that the `process_stream` function does not remove or alter lines ending with Markdown
/// hard line breaks.
fn test_preserve_hard_line_breaks() {
    let input = vec![
        "Line one with break.  ".to_string(),
        "Line two follows.".to_string(),
    ];
    let output = process_stream(&input);
    assert_eq!(output.len(), 2);
    assert_eq!(output[0], "Line one with break.");
    assert_eq!(output[1], "Line two follows.");
}

#[test]
/// Tests that `process_stream` preserves complex table formatting without modification.
///
/// This regression test ensures that properly formatted complex tables with multiple
/// columns and detailed content pass through the processing pipeline unchanged,
/// preventing regressions that might inadvertently alter correct formatting.
fn test_regression_complex_table() {
    let input: Vec<String> = include_str!("data/regression_table_input.txt")
        .lines()
        .map(str::to_string)
        .collect();
    let expected: Vec<String> = include_str!("data/regression_table_expected.txt")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(process_stream(&input), expected);
}

#[test]
fn test_renumber_basic() {
    let input = vec![
        "1. first".to_string(),
        "2. second".to_string(),
        "7. third".to_string(),
    ];
    let expected = vec!["1. first", "2. second", "3. third"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_renumber_with_fence() {
    let input = vec![
        "1. item".to_string(),
        "```".to_string(),
        "code".to_string(),
        "```".to_string(),
        "9. next".to_string(),
    ];
    let expected = vec![
        "1. item".to_string(),
        "```".to_string(),
        "code".to_string(),
        "```".to_string(),
        "2. next".to_string(),
    ];
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_cli_renumber_option() {
    let output = Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--renumber")
        .write_stdin("1. a\n4. b\n")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(text, "1. a\n2. b\n");
}

#[test]
fn test_renumber_nested_lists() {
    let input = vec![
        "1. first",
        "    1. sub first",
        "    3. sub second",
        "2. second",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();

    let expected = vec![
        "1. first",
        "    1. sub first",
        "    2. sub second",
        "2. second",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();

    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_renumber_tabs_in_indent() {
    let input = vec!["1. first", "\t1. sub first", "\t5. sub second", "2. second"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

    let expected = vec!["1. first", "\t1. sub first", "\t2. sub second", "2. second"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_renumber_mult_paragraph_items() {
    let input = vec!["1. first", "", "    still first paragraph", "", "2. second"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

    let expected = vec!["1. first", "", "    still first paragraph", "", "2. second"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_renumber_table_in_list() {
    let input = vec!["1. first", "    | A | B |", "    | 1 | 2 |", "5. second"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

    let expected = vec!["1. first", "    | A | B |", "    | 1 | 2 |", "2. second"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_format_breaks_basic() {
    let input = vec!["foo", "***", "bar"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let expected = vec![
        "foo".to_string(),
        "_".repeat(THEMATIC_BREAK_LEN),
        "bar".to_string(),
    ];
    assert_eq!(format_breaks(&input), expected);
}

#[test]
fn test_format_breaks_ignores_code() {
    let input = vec!["```", "---", "```"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(format_breaks(&input), input);
}

#[test]
fn test_format_breaks_mixed_chars() {
    let input = vec!["-*-*-"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(format_breaks(&input), input);
}

#[test]
fn test_format_breaks_with_spaces_and_indent() {
    let input = vec!["  -  -  -  "]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let expected = vec!["_".repeat(THEMATIC_BREAK_LEN)];
    assert_eq!(format_breaks(&input), expected);
}

#[test]
fn test_format_breaks_with_tabs_and_underscores() {
    let input = vec!["\t_\t_\t_\t"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let expected = vec!["_".repeat(THEMATIC_BREAK_LEN)];
    assert_eq!(format_breaks(&input), expected);
}

#[test]
fn test_cli_breaks_option() {
    let output = Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--breaks")
        .write_stdin("---\n")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{}\n", "_".repeat(THEMATIC_BREAK_LEN))
    );
}
