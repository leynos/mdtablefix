//! Tests for the checked row and width access in `calculate_widths` and
//! `format_rows`.
//!
//! Both helpers take a width vector whose length is meant to match the widest
//! row, and both now decline to index past either end rather than panicking:
//! `calculate_widths` skips a cell whose column has no width slot, and
//! `format_rows` pads a cell against a default width when its slot is absent.
//!
//! Every other test in this module feeds them lengths that already agree, so
//! the decline paths would otherwise never run. These cases construct the
//! mismatch deliberately — a row wider than `max_cols`, and a widths slice
//! shorter than a row — which is the only way either guard is reachable.

use rstest::rstest;

use super::*;

/// A row may carry more cells than the table declares columns.
///
/// `calculate_widths` must size the result to `max_cols` regardless, ignoring
/// the surplus cells rather than indexing past the end of `widths`.
#[rstest]
#[case::one_extra_cell(vec!["a".to_owned(), "bb".to_owned(), "ccc".to_owned()], 2, vec![1, 2])]
#[case::three_extra_cells(vec!["aa".to_owned(), "b".to_owned(), "cccc".to_owned(), "d".to_owned()], 2, vec![2, 1])]
#[case::all_cells_overflow(vec!["aaa".to_owned()], 0, vec![])]
fn calculate_widths_ignores_cells_beyond_max_cols(
    #[case] row: Vec<String>,
    #[case] max_cols: usize,
    #[case] expected: Vec<usize>,
) {
    let widths = calculate_widths(std::slice::from_ref(&row), max_cols);

    assert_eq!(
        widths, expected,
        "row {row:?} at max_cols {max_cols} must size widths to max_cols and ignore surplus cells",
    );
}

/// The surplus cells of one row must not disturb the columns of another.
///
/// This is the realistic shape: a ragged table where one row is longer than
/// the declared column count. The width of each declared column comes from the
/// cells that actually have a slot.
#[test]
fn calculate_widths_keeps_declared_columns_when_a_later_row_overflows() {
    let rows = vec![
        vec!["ab".to_owned(), "c".to_owned()],
        vec!["d".to_owned(), "efg".to_owned(), "ignored".to_owned()],
    ];

    let widths = calculate_widths(&rows, 2);

    assert_eq!(
        widths,
        vec![2, 3],
        "column 0 is max(2,1) and column 1 is max(1,3); the third cell of the second row has no \
         slot and must be skipped",
    );
}

/// A widths slice shorter than a row must not panic; the missing columns pad
/// against zero.
///
/// `pad_cell_to_width` never truncates, so a zero target leaves the escaped
/// cell exactly as it is. That is the documented decline: no width is
/// invented, and no cell is dropped.
#[rstest]
#[case::one_column_short(&[3], "ab", "cd", "| ab  | cd |")]
#[case::empty_widths(&[], "ab", "cd", "| ab | cd |")]
fn format_rows_pads_columns_without_a_width_slot(
    #[case] widths: &[usize],
    #[case] first: &str,
    #[case] second: &str,
    #[case] expected: &str,
) {
    let rows = vec![vec![first.to_owned(), second.to_owned()]];

    let formatted = format_rows(&rows, widths, "");

    assert_eq!(
        formatted,
        vec![expected.to_owned()],
        "a widths slice of {widths:?} must pad the unmatched column against zero rather than panic",
    );
}

/// Cells that do have a width slot must still be padded correctly when a
/// later column does not.
///
/// Without this, the property above would also pass if `format_rows` ignored
/// widths entirely.
#[test]
fn format_rows_still_pads_columns_that_have_a_width_slot() {
    let rows = vec![vec!["ab".to_owned(), "cd".to_owned()]];

    let formatted = format_rows(&rows, &[4], "");

    assert_eq!(
        formatted,
        vec!["| ab   | cd |".to_owned()],
        "column 0 has width 4 and must pad; column 1 has no slot and must not",
    );
}

proptest! {
    /// No combination of row width, declared columns and widths length may
    /// panic in either helper.
    ///
    /// The mismatch is generated directly rather than filtered, so every
    /// sample reaches at least one of the two guards whenever the lengths
    /// disagree — which is the whole class this layer hardens.
    #[test]
    fn reflow_helpers_tolerate_mismatched_column_counts(
        cell_count in 0usize..6,
        max_cols in 0usize..6,
        width_count in 0usize..6,
        cell_text in proptest::collection::vec("[a-z]{0,4}", 0..6),
    ) {
        let row: Vec<String> = cell_text
            .into_iter()
            .cycle()
            .take(cell_count)
            .collect();
        let widths: Vec<usize> = (0..width_count).map(|index| index % 4).collect();
        let rows = std::slice::from_ref(&row);

        let computed = calculate_widths(rows, max_cols);
        let formatted = format_rows(rows, &widths, "");

        // The contract is about shape, not content: whatever the mismatch,
        // `calculate_widths` reports exactly one width per declared column and
        // `format_rows` reports exactly one line per row.
        prop_assert_eq!(
            computed.len(),
            max_cols,
            "calculate_widths must report one width per declared column for \
             {} cells at max_cols {}",
            cell_count,
            max_cols,
        );
        prop_assert_eq!(
            formatted.len(),
            rows.len(),
            "format_rows must report one line per row for widths {:?}",
            widths,
        );
    }
}
