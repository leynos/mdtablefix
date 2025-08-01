//! Markdown table reflow utilities.
//!
//! Implements the algorithm outlined in
//! [`docs/architecture.md`](../../docs/architecture.md).
//! Provides helpers used by the `reflow` module and `reflow_table` itself.

use regex::Regex;

static ESCAPED_PIPE_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\\\|").unwrap());

#[must_use]
/// Split a Markdown table row into individual cell strings.
///
/// Escaped pipe characters (`\|`) are treated as literals and whitespace
/// inside each cell is trimmed.
///
/// # Examples
///
/// ```
/// use mdtablefix::split_cells;
/// assert_eq!(
///     split_cells("| A | B |"),
///     vec!["A".to_string(), "B".to_string()]
/// );
/// assert_eq!(
///     split_cells("a | b \\| c | d"),
///     vec!["a".to_string(), "b | c".to_string(), "d".to_string()]
/// );
/// ```
pub fn split_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim().trim_start_matches('|').trim_end_matches('|');
    let placeholder = '\u{1f}';
    let replaced = ESCAPED_PIPE_RE.replace_all(trimmed, &placeholder.to_string());
    replaced
        .split('|')
        .map(|cell| cell.trim().replace(placeholder, "|"))
        .collect()
}

pub(crate) fn format_separator_cells(widths: &[usize], sep_cells: &[String]) -> Vec<String> {
    if sep_cells.len() != widths.len() {
        return sep_cells.to_vec();
    }

    sep_cells
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            let trimmed = cell.trim();
            let left = trimmed.starts_with(':');
            let right = trimmed.ends_with(':');
            let mut dashes = "-".repeat(widths[i].max(3));
            if left {
                dashes.remove(0);
                dashes.insert(0, ':');
            }
            if right {
                dashes.pop();
                dashes.push(':');
            }
            dashes
        })
        .collect()
}

fn sep_index_within(idx: Option<usize>, len: usize) -> Option<usize> {
    match idx {
        Some(i) if i < len => Some(i),
        _ => None,
    }
}

fn rows_mismatched(rows: &[Vec<String>], split_within_line: bool) -> bool {
    if split_within_line {
        return false;
    }
    let Some(first_len) = rows.first().map(Vec::len) else {
        return false;
    };
    rows.iter()
        .skip(1)
        .any(|row| row.len() != first_len && !row.iter().all(|c| SEP_RE.is_match(c)))
}

pub(crate) static SEP_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^[\s|:-]+$").unwrap());

/// Holds the parsed and validated table data.
///
/// This is produced by [`parse_and_validate`] and passed to
/// [`calculate_and_format`].
///
/// * `cleaned` - rows after empty cells are removed
/// * `output_rows` - rows ready for output (separator removed)
/// * `sep_cells` - optional separator cells for formatting
/// * `max_cols` - maximum column count across all rows
struct ParsedTable {
    cleaned: Vec<Vec<String>>,
    output_rows: Vec<Vec<String>>,
    sep_cells: Option<Vec<String>>,
    max_cols: usize,
}

/// Extracts the leading whitespace of the first line and returns trimmed lines.
///
/// Lines beginning with `\-` are removed after trimming. These lines escape a
/// leading pipe marker and should not be part of the table.
fn extract_indent_and_trim(lines: &[String]) -> (String, Vec<String>) {
    let indent: String = lines[0].chars().take_while(|c| c.is_whitespace()).collect();
    let trimmed = lines
        .iter()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.trim_start().starts_with("\\-"))
        .collect();
    (indent, trimmed)
}

/// Removes and return the first separator line detected in `lines`.
fn extract_separator_line(lines: &mut Vec<String>) -> Option<String> {
    let sep_idx = lines.iter().position(|l| SEP_RE.is_match(l));
    sep_idx.map(|idx| lines.remove(idx))
}

/// Parses table rows and validates column consistency.
fn parse_and_validate(trimmed: &[String], sep_line: Option<&String>) -> Option<ParsedTable> {
    let (rows, split_within_line) = crate::reflow::parse_rows(trimmed);
    let max_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let (sep_cells, sep_row_idx) = crate::reflow::detect_separator(sep_line, &rows, max_cols);
    let cleaned = crate::reflow::clean_rows(rows);
    if rows_mismatched(&cleaned, split_within_line) {
        return None;
    }
    let mut output_rows = cleaned.clone();
    if let Some(idx) = sep_index_within(sep_row_idx, output_rows.len()) {
        output_rows.remove(idx);
    }
    Some(ParsedTable {
        cleaned,
        output_rows,
        sep_cells,
        max_cols,
    })
}

/// Calculates column widths and formats the final table output.
fn calculate_and_format(
    cleaned: &[Vec<String>],
    output_rows: &[Vec<String>],
    sep_cells: Option<Vec<String>>,
    max_cols: usize,
    indent: &str,
) -> Vec<String> {
    let widths = crate::reflow::calculate_widths(cleaned, max_cols);
    let out = crate::reflow::format_rows(output_rows, &widths, indent);
    crate::reflow::insert_separator(out, sep_cells, &widths, indent)
}

/// Reflow a Markdown table so columns align uniformly.
///
/// Invalid tables are returned unchanged.
///
/// # Examples
///
/// ```
/// use mdtablefix::reflow_table;
/// let lines = vec![
///     "| A | B |    |".to_string(),
///     "| 1 | 2 |  | 3 | 4 |".to_string(),
/// ];
/// let expected = vec![
///     "| A | B |".to_string(),
///     "| 1 | 2 |".to_string(),
///     "| 3 | 4 |".to_string(),
/// ];
/// assert_eq!(reflow_table(&lines), expected);
/// ```
#[must_use]
pub fn reflow_table(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }

    let (indent, mut trimmed) = extract_indent_and_trim(lines);
    let sep_line = extract_separator_line(&mut trimmed);

    let Some(parsed) = parse_and_validate(&trimmed, sep_line.as_ref()) else {
        return lines.to_vec();
    };

    calculate_and_format(
        &parsed.cleaned,
        &parsed.output_rows,
        parsed.sep_cells,
        parsed.max_cols,
        &indent,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sep_index_within_bounds() {
        assert_eq!(sep_index_within(Some(1), 3), Some(1));
        assert_eq!(sep_index_within(Some(3), 3), None);
        assert_eq!(sep_index_within(None, 3), None);
    }

    #[test]
    fn detect_row_mismatch() {
        let rows = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["1".to_string(), "2".to_string()],
        ];
        assert!(!rows_mismatched(&rows, false));

        let mismatch = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["1".to_string()],
        ];
        assert!(rows_mismatched(&mismatch, false));

        let with_sep = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["---".to_string(), "---".to_string()],
            vec!["1".to_string(), "2".to_string()],
        ];
        assert!(!rows_mismatched(&with_sep, false));

        assert!(!rows_mismatched(&mismatch, true));
    }
}
