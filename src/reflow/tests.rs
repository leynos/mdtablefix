//! Tests for the table reflow helper module.

use rstest::rstest;

use super::*;

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
