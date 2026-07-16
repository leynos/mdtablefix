//! Provenance-aware recovery of logical table rows from physical source lines.

use super::{LEADING_EMPTY_CELL_MARKER, SEP_RE};

pub(super) fn cell_is_semantically_empty(cell: &str) -> bool {
    cell.is_empty() || cell == LEADING_EMPTY_CELL_MARKER
}

pub(super) fn split_physical_rows(mut physical_rows: Vec<Vec<String>>) -> (Vec<Vec<String>>, bool) {
    let expected_width = infer_expected_width(&physical_rows);
    if let Some(first_row) = physical_rows.first_mut()
        && expected_width < first_row.len()
        && first_row[expected_width..]
            .iter()
            .all(|cell| cell_is_semantically_empty(cell))
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

fn infer_expected_width(rows: &[Vec<String>]) -> usize {
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

fn has_embedded_separator_row(row: &[String], width: usize) -> bool {
    if !is_concatenated_rows(row, width) {
        return false;
    }
    let row_count = (row.len() + 1) / (width + 1);
    (0..row_count).any(|index| {
        let start = index * (width + 1);
        row[start..start + width]
            .iter()
            .all(|cell| cell.contains('-') && SEP_RE.is_match(cell))
    })
}

fn is_concatenated_rows(row: &[String], width: usize) -> bool {
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

fn append_concatenated_rows(logical_rows: &mut Vec<Vec<String>>, row: Vec<String>, width: usize) {
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
