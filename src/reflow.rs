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
///
/// # Arguments
///
/// - `trimmed`: Trimmed table lines collected from the source document.
///
/// # Returns
///
/// A tuple containing the parsed rows and a flag indicating whether the
/// sentinel split crossed an original line boundary.
///
/// # Examples
///
/// ```rust,ignore
/// let trimmed = vec!["| a | b |".to_string(), "| 1 | 2 |".to_string()];
/// let (rows, split_within_line) = mdtablefix::reflow::parse_rows(&trimmed);
///
/// assert_eq!(
///     rows,
///     vec![
///         vec!["a".to_string(), "b".to_string()],
///         vec!["1".to_string(), "2".to_string()],
///     ]
/// );
/// assert!(!split_within_line);
/// ```
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
///
/// # Arguments
///
/// - `rows`: Parsed rows that may still contain continuation markers.
///
/// # Returns
///
/// Rows with marker cells restored to empty strings and fully empty rows
/// removed.
///
/// # Examples
///
/// ```rust,ignore
/// let rows = vec![
///     vec!["\u{1d}".to_string(), "value".to_string()],
///     vec![String::new(), String::new()],
/// ];
/// let cleaned = mdtablefix::reflow::clean_rows(rows);
///
/// assert_eq!(cleaned, vec![vec![String::new(), "value".to_string()]]);
/// ```
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
///
/// # Arguments
///
/// - `rows`: Parsed rows whose cell widths should contribute to the final table layout.
/// - `max_cols`: The number of output columns to size.
///
/// # Returns
///
/// A vector containing the widest emitted display width for each column.
///
/// # Examples
///
/// ```rust,ignore
/// let rows = vec![
///     vec!["ASCII".to_string(), "漢".to_string()],
///     vec!["a | b".to_string(), "wide".to_string()],
/// ];
/// let widths = mdtablefix::reflow::calculate_widths(&rows, 2);
///
/// assert_eq!(widths, vec![6, 4]);
/// ```
pub(crate) fn calculate_widths(rows: &[Vec<String>], max_cols: usize) -> Vec<usize> {
    let mut widths = vec![0; max_cols];
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(emitted_cell_width(cell));
        }
    }
    widths
}

/// Formats each row with the supplied display widths and original indentation.
///
/// # Arguments
///
/// - `rows`: Output rows to emit.
/// - `widths`: Display widths calculated for each column.
/// - `indent`: Leading whitespace that should prefix every emitted row.
///
/// # Returns
///
/// Fully formatted table rows with escaped literal pipes and padding applied.
///
/// # Examples
///
/// ```rust,ignore
/// let rows = vec![vec!["a".to_string(), "b | c".to_string()]];
/// let widths = vec![1, 5];
/// let formatted = mdtablefix::reflow::format_rows(&rows, &widths, "  ");
///
/// assert_eq!(formatted, vec!["  | a | b \\| c |".to_string()]);
/// ```
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
///
/// # Arguments
///
/// - `out`: Formatted table body rows.
/// - `sep_cells`: Optional separator cells to reinsert.
/// - `widths`: Display widths for each column.
/// - `indent`: Leading whitespace to prefix the separator row.
///
/// # Returns
///
/// The output rows with a formatted separator inserted after the header row
/// when separator cells are available.
///
/// # Examples
///
/// ```rust,ignore
/// let out = vec!["| head | body |".to_string(), "| row  | text |".to_string()];
/// let inserted = mdtablefix::reflow::insert_separator(
///     out,
///     Some(vec!["---".to_string(), ":--".to_string()]),
///     &[4, 4],
///     "",
/// );
///
/// assert_eq!(
///     inserted,
///     vec![
///         "| head | body |".to_string(),
///         "| ---- | :--- |".to_string(),
///         "| row  | text |".to_string(),
///     ]
/// );
/// ```
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
///
/// # Arguments
///
/// - `sep_line`: Optional separator line extracted from the original input.
/// - `rows`: Parsed table rows.
/// - `max_cols`: The maximum column count across the parsed rows.
///
/// # Returns
///
/// A tuple containing the chosen separator cells and the index of the
/// promoted separator row when one of the parsed rows is reused.
///
/// # Examples
///
/// ```rust,ignore
/// let rows = vec![
///     vec!["head".to_string(), "body".to_string()],
///     vec!["---".to_string(), "---".to_string()],
///     vec!["row".to_string(), "text".to_string()],
/// ];
/// let (sep_cells, sep_row_idx) = mdtablefix::reflow::detect_separator(None, &rows, 2);
///
/// assert_eq!(sep_cells, Some(vec!["---".to_string(), "---".to_string()]));
/// assert_eq!(sep_row_idx, Some(1));
/// ```
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
                escape_literal_pipes(&cell)
            }
        })
        .collect::<Vec<_>>();

    format!("| {} |", protected_cells.join(" | "))
}

fn pad_cell_to_width(cell: &str, width: usize) -> String {
    let escaped = escape_literal_pipes(cell);
    let padding = width.saturating_sub(emitted_cell_width(cell));
    format!("{escaped}{}", " ".repeat(padding))
}

fn escape_literal_pipes(cell: &str) -> String { cell.replace('|', r"\|") }

fn emitted_cell_width(cell: &str) -> usize {
    UnicodeWidthStr::width(escape_literal_pipes(cell).as_str())
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
}
