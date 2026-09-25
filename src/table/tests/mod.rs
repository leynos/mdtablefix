//! Unit tests for table parsing and formatting.
//!
//! These tests compile as a child module of `table`, so they can cover
//! private parsing helpers without keeping test-only code in the production
//! module.

use rstest::rstest;

use super::*;

mod split_cells;

#[test]
fn sep_index_within_bounds() {
    assert_eq!(sep_index_within(Some(1), 3), Some(1));
    assert_eq!(sep_index_within(Some(3), 3), None);
    assert_eq!(sep_index_within(None, 3), None);
}

#[test]
fn reflow_table_preserves_leading_empty_marker_character_as_payload() {
    let lines = vec![
        "| Header |".to_owned(),
        "| --- |".to_owned(),
        "| \u{1d} |".to_owned(),
    ];

    assert_eq!(
        reflow_table(&lines),
        vec![
            "| Header |".to_owned(),
            "| ------ |".to_owned(),
            "| \u{1d}      |".to_owned(),
        ]
    );
}

#[test]
fn reflow_table_preserves_escaped_pipe_sentinel_character_as_payload() {
    let lines = vec![
        "| Header | Value |".to_owned(),
        "| --- | --- |".to_owned(),
        "| \u{1f} | data |".to_owned(),
    ];

    assert_eq!(
        reflow_table(&lines),
        vec![
            "| Header | Value |".to_owned(),
            "| ------ | ----- |".to_owned(),
            "| \u{1f}      | data  |".to_owned(),
        ]
    );
}

#[test]
fn detect_row_mismatch() {
    let rows = vec![
        vec!["a".to_owned(), "b".to_owned()],
        vec!["1".to_owned(), "2".to_owned()],
    ];
    assert!(!rows_mismatched(&rows, false));

    let mismatch = vec![vec!["a".to_owned(), "b".to_owned()], vec!["1".to_owned()]];
    assert!(rows_mismatched(&mismatch, false));

    let with_sep = vec![
        vec!["a".to_owned(), "b".to_owned()],
        vec!["---".to_owned(), "---".to_owned()],
        vec!["1".to_owned(), "2".to_owned()],
    ];
    assert!(!rows_mismatched(&with_sep, false));

    assert!(!rows_mismatched(&mismatch, true));
}

#[rstest]
#[case(vec![2], vec!["---".to_owned()], vec!["---".to_owned()])]
#[case(vec![5], vec![":---".to_owned()], vec![":----".to_owned()])]
#[case(vec![5], vec!["---:".to_owned()], vec!["----:".to_owned()])]
#[case(vec![5], vec![":--:".to_owned()], vec![":---:".to_owned()])]
fn format_separator_cells_preserves_alignment_markers(
    #[case] widths: Vec<usize>,
    #[case] cells: Vec<String>,
    #[case] expected: Vec<String>,
) {
    assert_eq!(format_separator_cells(&widths, &cells), expected);
}

#[test]
fn format_separator_cells_returns_empty_when_counts_mismatch() {
    let sep_cells = vec!["---".to_owned()];

    assert!(format_separator_cells(&[3, 4], &sep_cells).is_empty());
}

#[test]
fn reflow_table_returns_lone_single_cell_line_unchanged() {
    // A single pipe-prefixed line with no separator row is a stray pipe
    // (for example a shell pipeline continuation), not a table. It must pass
    // through verbatim rather than gaining a fabricated trailing pipe.
    let lines = vec!["| tee /tmp/test.log".to_owned()];

    assert_eq!(reflow_table(&lines), lines);
}

#[test]
fn reflow_table_keeps_the_delimiter_row_of_a_table_with_an_empty_header() {
    // `|  |  |` is made only of pipes and spaces, so `SEP_RE` matches it.
    // Taking the empty header for the delimiter row demoted the real
    // delimiter row to a data row and left two delimiter-shaped rows for
    // later passes to consume in turn, so the table never settled.
    let lines = vec![
        "|  |  |".to_owned(),
        "| --- | --- |".to_owned(),
        "| a |  |".to_owned(),
    ];

    assert_eq!(
        reflow_table(&lines),
        vec!["| a   |     |".to_owned(), "| --- | --- |".to_owned()]
    );
}

#[test]
fn reflow_table_keeps_the_genuine_delimiter_row_below_a_malformed_header() {
    // `| - - |` is made only of pipes, spaces, and dashes, so `SEP_RE` matches
    // it and the old per-cell test found a dash in its only cell. The malformed
    // header was then taken for the delimiter row and rewritten into `| --- |`,
    // while the genuine delimiter row below it was laid out as text. A
    // delimiter cell admits no embedded whitespace, so the genuine row is the
    // one extracted and the malformed row stays data.
    let lines = vec![
        "| - - |".to_owned(),
        "| --- |".to_owned(),
        "| a |".to_owned(),
    ];
    let mut scannable = lines.clone();

    assert_eq!(
        extract_separator_line(&mut scannable),
        Some("| --- |".to_owned())
    );
    assert_eq!(scannable, vec!["| - - |".to_owned(), "| a |".to_owned()]);
    assert_eq!(
        reflow_table(&lines),
        vec![
            "| - - |".to_owned(),
            "| --- |".to_owned(),
            "| a   |".to_owned(),
        ]
    );
}

#[rstest]
#[case::dashes("| --- | --- |", true)]
#[case::alignment_markers("| :--: | ---: |", true)]
#[case::single_column("---", true)]
#[case::left_alignment("| :-- |", true)]
#[case::right_alignment("| --: |", true)]
#[case::centre_alignment("| :--: |", true)]
#[case::empty_header("|  |  |", false)]
#[case::lone_dash_header("|  | - |", false)]
#[case::spaced_dashes("| - - |", false)]
#[case::spaced_alignment_markers("| :- : |", false)]
#[case::content_row("| a | b |", false)]
fn is_delimiter_row_requires_a_dash_in_every_cell(#[case] line: &str, #[case] expected: bool) {
    assert_eq!(is_delimiter_row(line), expected);
}

#[test]
fn reflow_table_keeps_a_header_whose_lone_dash_tempts_the_delimiter_scan() {
    // The shrunk counter-example the issue #493 sweep raised: a header row
    // made only of pipes, spaces, and one dash satisfies `SEP_RE` and
    // carries a dash, so a line-level test took it for the delimiter row.
    // The real delimiter row was then laid out as text and the synthesized
    // one measured a column wider on every pass.
    let lines = vec![
        "|  | - |".to_owned(),
        "| --- | --- |".to_owned(),
        "|  | aAAa |".to_owned(),
    ];
    let expected = vec![
        "|     | -    |".to_owned(),
        "| --- | ---- |".to_owned(),
        "|     | aAAa |".to_owned(),
    ];

    assert_eq!(reflow_table(&lines), expected);
    assert_eq!(
        reflow_table(&expected),
        expected,
        "reflow is not a fixed point"
    );
}

#[test]
fn reflow_table_returns_original_lines_for_mismatched_separator_columns() {
    let lines = vec![
        "| head |".to_owned(),
        "| --- | --- |".to_owned(),
        "| body |".to_owned(),
    ];

    assert_eq!(reflow_table(&lines), lines);
}
