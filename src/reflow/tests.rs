//! Tests for the table reflow helper module.

use proptest::prelude::*;
use rstest::rstest;
use tracing_test::traced_test;

use super::*;

fn single_line_character_strategy() -> impl Strategy<Value = char> {
    any::<char>().prop_filter("table cells must remain on one source line", |character| {
        !matches!(character, '\r' | '\n' | '\u{1d}' | '\u{1f}')
    })
}

fn arbitrary_non_empty_cell_strategy() -> BoxedStrategy<String> {
    prop_oneof![
        2 => Just("ROW_END".to_string()),
        2 => Just("|".to_string()),
        1 => Just("left | right".to_string()),
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
            let separator = vec!["---".to_string(); column_count];
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

fn normalize_markers(rows: &[Vec<String>]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| {
            let mut has_seen_content = false;
            row.iter()
                .map(|cell| {
                    if cell == LEADING_EMPTY_CELL_MARKER {
                        assert!(
                            !has_seen_content,
                            "continuation marker must remain in the leading empty-cell run"
                        );
                        String::new()
                    } else {
                        has_seen_content |= !cell.is_empty();
                        cell.clone()
                    }
                })
                .collect()
        })
        .collect()
}

#[test]
fn parse_rows_preserves_literal_row_end_cell() {
    let input = vec![
        "| Name | Value |".to_string(),
        "| marker | ROW_END |".to_string(),
    ];

    assert_eq!(
        parse_rows(&input),
        (
            vec![
                vec!["Name".to_string(), "Value".to_string()],
                vec!["marker".to_string(), "ROW_END".to_string()],
            ],
            false,
        )
    );
}

#[test]
fn parse_rows_recovers_legacy_rows_with_embedded_separator() {
    let input = vec!["| Name | Notes |  | --- | --- |  | alpha | value |".to_string()];

    assert_eq!(
        parse_rows(&input),
        (
            vec![
                vec!["Name".to_string(), "Notes".to_string()],
                vec!["---".to_string(), "---".to_string()],
                vec!["alpha".to_string(), "value".to_string()],
            ],
            true,
        )
    );
}

#[test]
fn parse_rows_preserves_adjacent_empty_interior_cell() {
    let input = vec!["| A || C |".to_string()];

    assert_eq!(
        parse_rows(&input),
        (
            vec![vec!["A".to_string(), String::new(), "C".to_string()]],
            false,
        )
    );
}

#[test]
fn parse_rows_preserves_whitespace_padded_empty_cells() {
    let input = vec!["| A |  | C |".to_string()];

    assert_eq!(
        parse_rows(&input),
        (
            vec![vec!["A".to_string(), String::new(), "C".to_string()]],
            false,
        )
    );
}

#[test]
fn parse_rows_preserves_trailing_empty_cells() {
    let input = vec!["| A | B | C |".to_string(), "| 1 | 2 |  |".to_string()];

    assert_eq!(
        parse_rows(&input),
        (
            vec![
                vec!["A".to_string(), "B".to_string(), "C".to_string()],
                vec!["1".to_string(), "2".to_string(), String::new()],
            ],
            false,
        )
    );
}

#[test]
fn parse_rows_splits_structural_rows_and_drops_marker_only_row() {
    let input = vec![
        "| H1 | H2 |  |".to_string(),
        "| A | B |  | C | D |".to_string(),
        "| | |".to_string(),
    ];

    let (rows, split_within_line) = parse_rows(&input);

    assert!(split_within_line);
    assert_eq!(
        rows,
        vec![
            vec!["H1".to_string(), "H2".to_string()],
            vec!["A".to_string(), "B".to_string()],
            vec!["C".to_string(), "D".to_string()],
        ]
    );
}

#[traced_test]
#[test]
fn parse_rows_logs_row_dimensions() {
    let input = vec!["| Name | Value |".to_string()];

    let _ = parse_rows(&input);

    assert!(logs_contain("parsed table row"));
    assert!(logs_contain("row_index=0"));
    assert!(logs_contain("cell_count=2"));
}

#[traced_test]
#[test]
fn empty_parsed_rows_log_discard_category() {
    let input = vec!["| | |".to_string()];
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
        let normalized = normalize_markers(&parsed);

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

        prop_assert_eq!(normalize_markers(&parsed), rows);
        prop_assert!(split_within_line);
    }
}

#[test]
fn protect_leading_empty_cells_reescapes_literal_pipes_after_marking() {
    let protected = protect_leading_empty_cells("|   | keep \\| literal | tail |");

    assert_eq!(
        split_cells(&protected),
        vec![
            LEADING_EMPTY_CELL_MARKER.to_string(),
            "keep | literal".to_string(),
            "tail".to_string(),
        ]
    );
}

#[test]
fn protect_leading_empty_cells_preserves_adjacent_interior_empty_cell() {
    let protected = protect_leading_empty_cells("| | ROW_END || ROW_END |");

    assert_eq!(
        split_cells(&protected),
        vec![
            LEADING_EMPTY_CELL_MARKER.to_string(),
            "ROW_END".to_string(),
            String::new(),
            "ROW_END".to_string(),
        ]
    );
}

#[test]
fn protect_leading_empty_cells_leaves_non_continuation_rows_unchanged() {
    let line = "| head | body \\| value |";

    assert_eq!(protect_leading_empty_cells(line), line);
}

#[test]
fn clean_rows_restores_markers_and_discards_empty_rows() {
    let rows = vec![
        vec![LEADING_EMPTY_CELL_MARKER.to_string(), "value".to_string()],
        vec![String::new(), String::new()],
    ];

    assert_eq!(
        clean_rows(rows),
        vec![vec![String::new(), "value".to_string()]]
    );
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
#[case(vec!["ASCII".to_string(), "wide".to_string()], vec!["narrow".to_string(), "text".to_string()], vec![6, 4])]
#[case(vec!["漢字".to_string(), "🙂".to_string()], vec!["é".to_string(), "emoji 🙂".to_string()], vec![4, 8])]
#[case(vec!["a | b".to_string()], vec!["plain".to_string()], vec![6])]
fn calculate_widths_uses_unicode_display_width(
    #[case] first: Vec<String>,
    #[case] second: Vec<String>,
    #[case] expected: Vec<usize>,
) {
    let rows = vec![first, second];

    assert_eq!(calculate_widths(&rows, expected.len()), expected);
}

#[test]
fn format_rows_reescapes_literal_pipes_in_emitted_cells() {
    let rows = vec![vec![String::new(), "keep | literal".to_string()]];
    let widths = calculate_widths(&rows, 2);

    assert_eq!(
        format_rows(&rows, &widths, ""),
        vec!["|  | keep \\| literal |".to_string()]
    );
}
