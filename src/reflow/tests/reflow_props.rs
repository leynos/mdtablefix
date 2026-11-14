//! Generated coverage for the reflow helpers that the layer changed.
//!
//! The checked-access guards in `calculate_widths`, `format_rows`,
//! `has_embedded_separator_row` and `is_concatenated_rows` declined rather than
//! panicked on out-of-range input, but only hand-written cases exercised them.
//! These properties derive the expected value from the contract instead of
//! restating the implementation, so an incorrect output fails the property
//! rather than merely failing to be reached.

use super::*;

/// Builds a cell carrying no leading-empty state.
fn plain_cell(payload: &str) -> Cell {
    Cell {
        payload: payload.to_owned(),
        leading_empty: false,
    }
}

/// Generates cell text that exercises escaping, padding and wide glyphs.
fn cell_text_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => Just(String::new()),
        2 => Just("|".to_owned()),
        2 => Just("漢".to_owned()),
        2 => Just("a|b".to_owned()),
        6 => "[a-z]{0,4}".prop_map(|text: String| text),
    ]
}

/// Generates a row whose width may differ from any other row's.
fn ragged_rows_strategy() -> impl Strategy<Value = Vec<Vec<String>>> {
    prop::collection::vec(prop::collection::vec(cell_text_strategy(), 0..5), 0..5)
}

proptest! {
    /// `calculate_widths` reports, per column, the widest in-range cell.
    ///
    /// `max_cols` is generated independently of the rows so it is sometimes
    /// shorter than a row. The expected vector is derived from `max_cols` and
    /// the cells that fall inside it, not from the function's own output.
    #[test]
    fn calculate_widths_matches_the_widest_in_range_cell(
        rows in ragged_rows_strategy(),
        max_cols in 0usize..6,
    ) {
        let widths = calculate_widths(&rows, max_cols);

        prop_assert_eq!(widths.len(), max_cols);
        for (column, width) in widths.iter().enumerate() {
            let expected = rows
                .iter()
                .filter_map(|row| row.get(column))
                .map(|cell| emitted_cell_width(cell))
                .max()
                .unwrap_or(0);
            prop_assert_eq!(
                *width, expected,
                "column {} of max_cols {} over {:?}", column, max_cols, rows
            );
        }
    }

    /// `format_rows` emits one line per row, padded to the supplied widths.
    ///
    /// The widths slice is generated independently of the rows, so it is
    /// sometimes shorter than a row and sometimes longer. A column with no
    /// width slot falls back to zero padding rather than panicking.
    #[test]
    fn format_rows_pads_every_column_or_falls_back_to_zero(
        rows in ragged_rows_strategy(),
        width_values in prop::collection::vec(0usize..8, 0..5),
        indent in prop_oneof![Just(""), Just("  ")],
    ) {
        let formatted = format_rows(&rows, &width_values, indent);

        prop_assert_eq!(formatted.len(), rows.len());
        for (line, row) in formatted.iter().zip(&rows) {
            prop_assert!(line.starts_with(indent), "{line:?} must carry the indent");
            let expected = row
                .iter()
                .enumerate()
                .map(|(column, cell)| {
                    pad_cell_to_width(cell, width_values.get(column).copied().unwrap_or_default())
                })
                .collect::<Vec<_>>()
                .join(" | ");
            prop_assert_eq!(line, &format!("{indent}| {expected} |"));
        }
    }

    /// A row with fewer than two entries is never a separator.
    ///
    /// An empty or single-row slice cannot carry a delimiter row below the
    /// header, so `detect_separator` must report no separator row index.
    ///
    /// Note on reachability: reverting only `detect_separator`'s inner
    /// `rows.get(1)` cannot make this property fail, because
    /// `should_use_second_row_as_separator` calls `second_row_is_separator`,
    /// which rejects a slice below two entries first. That guard is therefore
    /// unreachable defensive code by construction, and no mutation is claimed
    /// for it.
    #[test]
    fn detect_separator_reports_no_promoted_row_below_two_rows(
        rows in prop::collection::vec(prop::collection::vec(cell_text_strategy(), 0..4), 0..2),
        max_cols in 0usize..5,
    ) {
        let rows = rows
            .iter()
            .map(|row| row.iter().map(|text| plain_cell(text)).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let (_, sep_row_idx) = detect_separator(None, &rows, max_cols);

        prop_assert_eq!(sep_row_idx, None);
    }
}
