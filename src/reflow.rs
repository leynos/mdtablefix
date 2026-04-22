//! Helper functions for table reflow.
//!
//! The routines here parse raw rows, calculate cell widths, and format
//! aligned output for the main [`reflow_table`] function.

use regex::Regex;
use unicode_width::UnicodeWidthStr;

use crate::table::{SEP_RE, format_separator_cells, split_cells};

static SENTINEL_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\|\s*\|\s*").unwrap());
const LEADING_EMPTY_CELL_MARKER: &str = "\u{1d}";

/// Parses reflow input into rows while preserving continuation-cell boundaries.
///
/// Leading empty cells are protected before the global split so continuation
/// rows keep their original column positions.
pub(crate) fn parse_rows(trimmed: &[String]) -> (Vec<Vec<String>>, bool) {
    let protected = trimmed
        .iter()
        .map(|line| protect_leading_empty_cells(line))
        .collect::<Vec<_>>();
    let raw = protected.join(" ");
    let chunks: Vec<&str> = SENTINEL_RE.split(&raw).collect();
    let split_within_line = chunks.len() > trimmed.len();

    let cells = collect_cells(&chunks);
    let rows = split_into_rows(cells);

    (rows, split_within_line)
}

fn collect_cells(chunks: &[&str]) -> Vec<String> {
    let mut cells = Vec::new();
    for (idx, chunk) in chunks.iter().enumerate() {
        let mut ch = (*chunk).to_string();
        if idx != chunks.len() - 1 {
            ch.push_str(" |ROW_END|");
        }
        cells.extend(split_cells(&ch));
    }
    cells
}

fn split_into_rows(cells: Vec<String>) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut current = Vec::new();
    for cell in cells {
        if cell == "ROW_END" {
            if !current.is_empty() {
                rows.push(std::mem::take(&mut current));
            }
        } else {
            current.push(cell);
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }
    rows
}

/// Restores parser markers and removes rows that contain only empty cells.
pub(crate) fn clean_rows(rows: Vec<Vec<String>>) -> Vec<Vec<String>> {
    rows.into_iter()
        .map(|row| {
            row.into_iter()
                .map(|cell| {
                    if cell == LEADING_EMPTY_CELL_MARKER {
                        String::new()
                    } else {
                        cell
                    }
                })
                .collect::<Vec<_>>()
        })
        .filter(|row| row.iter().any(|cell| !cell.is_empty()))
        .collect()
}

/// Calculates display widths for each column across all parsed rows.
///
/// Widths are measured with `unicode-width` so wide glyphs align correctly in
/// the rendered table output.
pub(crate) fn calculate_widths(rows: &[Vec<String>], max_cols: usize) -> Vec<usize> {
    let mut widths = vec![0; max_cols];
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(UnicodeWidthStr::width(cell.as_str()));
        }
    }
    widths
}

/// Formats each row with the supplied display widths and original indentation.
pub(crate) fn format_rows(rows: &[Vec<String>], widths: &[usize], indent: &str) -> Vec<String> {
    rows.iter()
        .map(|row| {
            let padded: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(i, cell)| pad_cell_to_width(cell, widths[i]))
                .collect();
            format!("{}| {} |", indent, padded.join(" | "))
        })
        .collect()
}

/// Reinserts the separator row, when present, ahead of the table body.
pub(crate) fn insert_separator(
    out: Vec<String>,
    sep_cells: Option<Vec<String>>,
    widths: &[usize],
    indent: &str,
) -> Vec<String> {
    if let Some(mut cells) = sep_cells {
        while cells.len() < widths.len() {
            cells.push(String::new());
        }
        let sep_padded = format_separator_cells(widths, &cells);
        let sep_line_out = format!("{}| {} |", indent, sep_padded.join(" | "));
        if let Some(first) = out.first().cloned() {
            let mut with_sep = vec![first, sep_line_out];
            with_sep.extend(out.into_iter().skip(1));
            return with_sep;
        }
        return vec![sep_line_out];
    }
    out
}

/// Detects which row should act as the table separator, if any.
///
/// The explicit separator line is preferred, but the second parsed row can be
/// promoted when the source omitted a standalone separator line.
pub(crate) fn detect_separator(
    sep_line: Option<&String>,
    rows: &[Vec<String>],
    max_cols: usize,
) -> (Option<Vec<String>>, Option<usize>) {
    let mut sep_cells: Option<Vec<String>> = sep_line.map(|l| split_cells(l));
    let mut sep_row_idx: Option<usize> = None;

    let sep_invalid = invalid_separator(sep_cells.as_ref(), max_cols);
    if should_use_second_row_as_separator(sep_invalid, rows) {
        sep_cells = Some(rows[1].clone());
        sep_row_idx = Some(1);
    }

    (sep_cells, sep_row_idx)
}

fn invalid_separator(sep_cells: Option<&Vec<String>>, max_cols: usize) -> bool {
    match sep_cells {
        Some(c) => c.len() != max_cols,
        None => true,
    }
}

fn should_use_second_row_as_separator(sep_invalid: bool, rows: &[Vec<String>]) -> bool {
    sep_invalid && second_row_is_separator(rows)
}

fn second_row_is_separator(rows: &[Vec<String>]) -> bool {
    rows.len() > 1 && rows[1].iter().all(|c| SEP_RE.is_match(c))
}

/// Replaces leading empty cells with a marker so continuation rows survive the
/// global row-splitting pass.
fn protect_leading_empty_cells(line: &str) -> String {
    let cells = split_cells(line);
    let leading_empty_cells = cells.iter().take_while(|cell| cell.is_empty()).count();
    if leading_empty_cells == 0 {
        return line.to_string();
    }

    let protected_cells = cells
        .into_iter()
        .enumerate()
        .map(|(idx, cell)| {
            if idx < leading_empty_cells {
                LEADING_EMPTY_CELL_MARKER.to_string()
            } else {
                cell.replace('|', r"\|")
            }
        })
        .collect::<Vec<_>>();

    format!("| {} |", protected_cells.join(" | "))
}

fn pad_cell_to_width(cell: &str, width: usize) -> String {
    let padding = width.saturating_sub(UnicodeWidthStr::width(cell));
    format!("{cell}{}", " ".repeat(padding))
}

#[cfg(test)]
mod tests {
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

    #[rstest]
    #[case(vec!["ASCII".to_string(), "wide".to_string()], vec!["narrow".to_string(), "text".to_string()], vec![6, 4])]
    #[case(vec!["漢字".to_string(), "🙂".to_string()], vec!["é".to_string(), "emoji 🙂".to_string()], vec![4, 8])]
    fn calculate_widths_uses_unicode_display_width(
        #[case] first: Vec<String>,
        #[case] second: Vec<String>,
        #[case] expected: Vec<usize>,
    ) {
        let rows = vec![first, second];

        assert_eq!(calculate_widths(&rows, 2), expected);
    }
}
