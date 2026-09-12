//! Provenance-aware recovery of logical table rows from physical source lines.

use super::{Cell, SEP_RE};

/// Reports whether a cell has no content after accounting for leading emptiness.
pub(super) fn cell_is_semantically_empty(cell: &Cell) -> bool {
    cell.leading_empty || cell.payload.is_empty()
}

/// Splits physical rows that contain multiple logical rows and trims padding.
pub(super) fn split_physical_rows(mut physical_rows: Vec<Vec<Cell>>) -> (Vec<Vec<Cell>>, bool) {
    let expected_width = infer_expected_width(&physical_rows);
    if let Some(first_row) = physical_rows.first_mut()
        && expected_width < first_row.len()
        && first_row[expected_width..]
            .iter()
            .all(cell_is_semantically_empty)
    {
        first_row.truncate(expected_width);
    }

    let mut logical_rows = Vec::new();
    let mut split_within_line = false;
    for row in physical_rows {
        if is_concatenated_rows(&row, expected_width) {
            split_within_line = true;
            append_concatenated_rows(&mut logical_rows, row, expected_width);
        } else {
            logical_rows.push(row);
        }
    }
    (logical_rows, split_within_line)
}

/// Infers the logical table width from the first row and concatenation markers.
fn infer_expected_width(rows: &[Vec<Cell>]) -> usize {
    let Some(first_row) = rows.first() else {
        return 0;
    };
    if let Some(width) =
        (1..first_row.len()).find(|width| has_embedded_separator_row(first_row, *width))
    {
        return width;
    }
    let non_empty_prefix = first_row
        .iter()
        .rposition(|cell| !cell_is_semantically_empty(cell))
        .map_or(0, |index| index + 1);
    let has_matching_concatenation = non_empty_prefix > 0
        && non_empty_prefix < first_row.len()
        && rows
            .iter()
            .skip(1)
            .any(|row| is_concatenated_rows(row, non_empty_prefix));

    if has_matching_concatenation {
        non_empty_prefix
    } else {
        first_row.len()
    }
}

/// Reports whether a concatenated row contains an embedded separator row.
fn has_embedded_separator_row(row: &[Cell], width: usize) -> bool {
    if !is_concatenated_rows(row, width) {
        return false;
    }
    let row_count = (row.len() + 1) / (width + 1);
    (0..row_count).any(|index| {
        let start = index * (width + 1);
        row[start..start + width]
            .iter()
            .all(|cell| cell.payload.contains('-') && SEP_RE.is_match(&cell.payload))
    })
}

/// Reports whether a physical row encodes multiple logical rows.
fn is_concatenated_rows(row: &[Cell], width: usize) -> bool {
    if width == 0 || row.len() <= width || !(row.len() + 1).is_multiple_of(width + 1) {
        return false;
    }
    let row_count = (row.len() + 1) / (width + 1);
    row_count >= 2
        && (1..row_count).all(|index| cell_is_semantically_empty(&row[index * (width + 1) - 1]))
        && (0..row_count).all(|index| {
            let start = index * (width + 1);
            row[start..start + width]
                .iter()
                .any(|cell| !cell_is_semantically_empty(cell))
        })
}

/// Appends each logical row recovered from a concatenated physical row.
fn append_concatenated_rows(logical_rows: &mut Vec<Vec<Cell>>, row: Vec<Cell>, width: usize) {
    let mut cells = row.into_iter();
    loop {
        let logical_row = cells.by_ref().take(width).collect::<Vec<_>>();
        if logical_row.is_empty() {
            break;
        }
        logical_rows.push(logical_row);
        let _separator = cells.next();
    }
}
