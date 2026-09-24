//! Tests for the table reflow helper module.

use proptest::prelude::*;
use rstest::rstest;
// Wrapper over `tracing_test::traced_test`; see `test_macros` for why.
use test_macros::traced_test;

use super::*;

mod cell_parsing;

fn single_line_character_strategy() -> impl Strategy<Value = char> {
    any::<char>().prop_filter("table cells must remain on one source line", |character| {
        !matches!(character, '\r' | '\n')
    })
}

fn arbitrary_non_empty_cell_strategy() -> BoxedStrategy<String> {
    prop_oneof![
        2 => Just("ROW_END".to_owned()),
        2 => Just("|".to_owned()),
        1 => Just("left | right".to_owned()),
        8 => prop::collection::vec(single_line_character_strategy(), 0..=24)
            .prop_map(|characters| {
                let content = characters.into_iter().collect::<String>();
                format!("x{content}x")
            }),
    ]
    .boxed()
}

fn table_rows_strategy() -> impl Strategy<Value = Vec<Vec<String>>> {
    (2usize..=6).prop_flat_map(|column_count| {
        let first_row = generated_row_strategy(column_count, Just(column_count - 1));
        let remaining_rows =
            prop::collection::vec(generated_row_strategy(column_count, 0..column_count), 0..=7);
        (first_row, remaining_rows).prop_map(|(first_row, mut remaining_rows)| {
            let mut rows = vec![first_row];
            rows.append(&mut remaining_rows);
            rows
        })
    })
}

fn legacy_concatenated_rows_strategy() -> impl Strategy<Value = Vec<Vec<String>>> {
    (2usize..=6).prop_flat_map(|column_count| {
        let header = generated_row_strategy(column_count, 0..column_count);
        let body =
            prop::collection::vec(generated_row_strategy(column_count, 0..column_count), 1..=6);
        (header, body).prop_map(move |(header, body)| {
            let separator = vec!["---".to_owned(); column_count];
            std::iter::once(header)
                .chain(std::iter::once(separator))
                .chain(body)
                .collect()
        })
    })
}

fn generated_row_strategy(
    column_count: usize,
    non_empty_index: impl Strategy<Value = usize>,
) -> impl Strategy<Value = Vec<String>> {
    (
        prop::collection::vec(arbitrary_non_empty_cell_strategy(), column_count),
        prop::collection::vec(any::<bool>(), column_count),
        non_empty_index,
    )
        .prop_map(|(mut cells, empty_cell_flags, non_empty_index)| {
            for (index, (cell, is_empty)) in cells.iter_mut().zip(empty_cell_flags).enumerate() {
                if is_empty && index != non_empty_index {
                    cell.clear();
                }
            }
            cells
        })
}

fn render_table_row(row: &[String]) -> String {
    let escaped = row
        .iter()
        .map(|cell| escape_literal_pipes(cell))
        .collect::<Vec<_>>();
    format!("| {} |", escaped.join(" | "))
}

fn render_legacy_concatenated_rows(rows: &[Vec<String>]) -> String {
    let mut cells = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            cells.push(String::new());
        }
        cells.extend(row.iter().cloned());
    }
    render_table_row(&cells)
}

fn normalize_cells(rows: &[Vec<Cell>]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| {
            let mut has_seen_content = false;
            row.iter()
                .map(|cell| {
                    if cell.leading_empty {
                        assert!(
                            !has_seen_content,
                            "leading-empty state must remain in the leading empty-cell run"
                        );
                        String::new()
                    } else {
                        has_seen_content |= !cell.payload.is_empty();
                        cell.payload.clone()
                    }
                })
                .collect()
        })
        .collect()
}

#[test]
fn parse_rows_preserves_literal_row_end_cell() {
    let input = vec![
        "| Name | Value |".to_owned(),
        "| marker | ROW_END |".to_owned(),
    ];

    let (rows, split_within_line) = parse_rows(&input);

    assert_eq!(
        normalize_cells(&rows),
        vec![
            vec!["Name".to_owned(), "Value".to_owned()],
            vec!["marker".to_owned(), "ROW_END".to_owned()],
        ]
    );
    assert!(!split_within_line);
}

#[test]
fn parse_rows_recovers_legacy_rows_with_embedded_separator() {
    let input = vec!["| Name | Notes |  | --- | --- |  | alpha | value |".to_owned()];

    let (parsed, split_within_line) = parse_rows(&input);

    assert_eq!(
        normalize_cells(&parsed),
        vec![
            vec!["Name".to_owned(), "Notes".to_owned()],
            vec!["---".to_owned(), "---".to_owned()],
            vec!["alpha".to_owned(), "value".to_owned()],
        ]
    );
    assert!(split_within_line);
}

#[test]
fn parse_rows_preserves_adjacent_empty_interior_cell() {
    let input = vec!["| A || C |".to_owned()];

    let (parsed, split_within_line) = parse_rows(&input);

    assert_eq!(
        normalize_cells(&parsed),
        vec![vec!["A".to_owned(), String::new(), "C".to_owned()]]
    );
    assert!(!split_within_line);
}

#[test]
fn parse_rows_preserves_whitespace_padded_empty_cells() {
    let input = vec!["| A |  | C |".to_owned()];

    let (parsed, split_within_line) = parse_rows(&input);

    assert_eq!(
        normalize_cells(&parsed),
        vec![vec!["A".to_owned(), String::new(), "C".to_owned()]]
    );
    assert!(!split_within_line);
}

#[test]
fn parse_rows_preserves_trailing_empty_cells() {
    let input = vec!["| A | B | C |".to_owned(), "| 1 | 2 |  |".to_owned()];

    let (parsed, split_within_line) = parse_rows(&input);

    assert_eq!(
        normalize_cells(&parsed),
        vec![
            vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
            vec!["1".to_owned(), "2".to_owned(), String::new()],
        ]
    );
    assert!(!split_within_line);
}

#[test]
fn parse_rows_splits_structural_rows_and_drops_marker_only_row() {
    let input = vec![
        "| H1 | H2 |  |".to_owned(),
        "| A | B |  | C | D |".to_owned(),
        "| | |".to_owned(),
    ];

    let (rows, split_within_line) = parse_rows(&input);

    assert!(split_within_line);
    assert_eq!(
        normalize_cells(&rows),
        vec![
            vec!["H1".to_owned(), "H2".to_owned()],
            vec!["A".to_owned(), "B".to_owned()],
            vec!["C".to_owned(), "D".to_owned()],
        ]
    );
}

#[traced_test]
#[test]
fn parse_rows_logs_row_dimensions() {
    let input = vec!["| Name | Value |".to_owned()];

    let _ = parse_rows(&input);

    assert!(logs_contain("parsed table row"));
    assert!(logs_contain("row_index=0"));
    assert!(logs_contain("cell_count=2"));
}

#[traced_test]
#[test]
fn empty_parsed_rows_log_discard_category() {
    let input = vec!["| | |".to_owned()];
    let (rows, split_within_line) = parse_rows(&input);

    assert!(rows.is_empty());
    assert!(!split_within_line);

    assert!(logs_contain("discarded empty parsed row"));
    assert!(logs_contain("cell_count=2"));
    assert!(logs_contain("error_category=\"empty_row_discarded\""));
}

proptest! {
    #[test]
    fn parse_rows_keeps_generated_row_and_cell_boundaries(rows in table_rows_strategy()) {
        let input = rows
            .iter()
            .map(|row| render_table_row(row))
            .collect::<Vec<_>>();
        let (parsed, split_within_line) = parse_rows(&input);
        let normalized = normalize_cells(&parsed);

        prop_assert_eq!(normalized.len(), rows.len());
        let dimensions_match = normalized
            .iter()
            .zip(&rows)
            .all(|(actual, expected)| actual.len() == expected.len());
        prop_assert!(dimensions_match);
        prop_assert_eq!(normalized, rows);
        prop_assert!(!split_within_line);
    }


    #[test]
    fn parse_rows_recovers_generated_legacy_concatenated_rows(
        rows in legacy_concatenated_rows_strategy(),
    ) {
        let input = vec![render_legacy_concatenated_rows(&rows)];
        let (parsed, split_within_line) = parse_rows(&input);

        prop_assert_eq!(normalize_cells(&parsed), rows);
        prop_assert!(split_within_line);
    }
}

#[test]
fn escape_literal_pipes_only_escapes_bare_pipes() {
    assert_eq!(escape_literal_pipes("plain text"), "plain text");
    assert_eq!(escape_literal_pipes("left | right"), r"left \| right");
    assert_eq!(escape_literal_pipes(r"left \| right"), r"left \\| right");
}

#[test]
fn emitted_cell_width_accounts_for_escaping_and_unicode_width() {
    let ascii = "ASCII";
    let with_pipe = "a|b";
    let wide = "漢";

    assert_eq!(emitted_cell_width(ascii), ascii.len());
    assert_eq!(emitted_cell_width(with_pipe), with_pipe.len() + 1);
    assert_eq!(emitted_cell_width(wide), UnicodeWidthStr::width(wide));
}

#[test]
fn pad_cell_to_width_pads_short_cells_to_target_width() {
    let padded = pad_cell_to_width("cat", 5);

    assert_eq!(padded, "cat  ");
    assert_eq!(UnicodeWidthStr::width(padded.as_str()), 5);
}

#[test]
fn pad_cell_to_width_escapes_pipes_before_padding() {
    let padded = pad_cell_to_width("a|b", 5);

    assert_eq!(padded, r"a\|b ");
    assert_eq!(UnicodeWidthStr::width(padded.as_str()), 5);
}

#[test]
fn pad_cell_to_width_leaves_exact_width_cells_unpadded() {
    let cell = "漢";

    assert_eq!(pad_cell_to_width(cell, emitted_cell_width(cell)), cell);
}

#[test]
fn pad_cell_to_width_saturates_without_truncating() {
    assert_eq!(pad_cell_to_width("a|b", 2), r"a\|b");
}

#[rstest]
#[case(vec!["ASCII".to_owned(), "wide".to_owned()], vec!["narrow".to_owned(), "text".to_owned()], vec![6, 4])]
#[case(vec!["漢字".to_owned(), "🙂".to_owned()], vec!["é".to_owned(), "emoji 🙂".to_owned()], vec![4, 8])]
#[case(vec!["a | b".to_owned()], vec!["plain".to_owned()], vec![6])]
fn calculate_widths_uses_unicode_display_width(
    #[case] first: Vec<String>,
    #[case] second: Vec<String>,
    #[case] expected: Vec<usize>,
) {
    let rows = vec![first, second];

    assert_eq!(calculate_widths(&rows, expected.len()), expected);
}

/// Builds a parsed row from cell payloads that carry no leading-empty state.
fn parsed_row(payloads: &[&str]) -> Vec<Cell> {
    payloads
        .iter()
        .map(|payload| Cell {
            payload: (*payload).to_owned(),
            leading_empty: false,
        })
        .collect()
}

#[test]
fn second_row_is_not_a_separator_when_its_cells_carry_no_dash() {
    // `SEP_RE` matches a cell that is empty, so neither the row below a header
    // nor the row below it may be promoted to the delimiter row without a dash
    // in every cell; `row_parsing` requires the dash for the same reason. A row
    // of nothing but pipes and spaces was otherwise taken for the delimiter row,
    // which demoted the real one to a data row.
    let empty_cells = vec![
        parsed_row(&["head", "cells"]),
        parsed_row(&["", ""]),
        parsed_row(&["body", "cells"]),
    ];
    let delimiter = vec![parsed_row(&["head", "cells"]), parsed_row(&["---", "---"])];

    assert!(!second_row_is_separator(&empty_cells));
    assert!(second_row_is_separator(&delimiter));
}

#[test]
fn format_rows_reescapes_literal_pipes_in_emitted_cells() {
    let rows = vec![vec![String::new(), "keep | literal".to_owned()]];
    let widths = calculate_widths(&rows, 2);

    assert_eq!(
        format_rows(&rows, &widths, ""),
        vec!["|  | keep \\| literal |".to_owned()]
    );
}
