//! Markdown table reflow utilities.
//!
//! Implements the algorithm outlined in
//! [`docs/architecture.md`](../../docs/architecture.md).
//! Provides helpers used by the `reflow` module and `reflow_table` itself.

use regex::Regex;
use tracing::debug;

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
#[must_use]
pub fn split_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim().trim_start_matches('|');
    let trimmed = match trimmed.strip_suffix('|') {
        Some(without_pipe)
            if without_pipe
                .chars()
                .rev()
                .take_while(|character| *character == '\\')
                .count()
                % 2
                == 0 =>
        {
            without_pipe
        }
        _ => trimmed,
    };
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut characters = trimmed.chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '\\' if characters.peek() == Some(&'|') => {
                let _pipe = characters.next();
                cell.push('|');
            }
            '|' => {
                cells.push(cell.trim().to_string());
                cell.clear();
            }
            _ => cell.push(character),
        }
    }

    cells.push(cell.trim().to_string());
    cells
}

/// Formats separator cells so they match the computed table widths.
///
/// Alignment markers from the source separator are preserved while each cell
/// is widened to at least three dashes, as required by Markdown tables.
///
/// # Arguments
///
/// - `widths`: Computed display widths for each output column.
/// - `sep_cells`: Separator cells taken from the parsed Markdown table.
///
/// # Returns
///
/// Separator cells widened to the target widths while preserving left and
/// right alignment markers. When the counts do not match, an empty vector is
/// returned so callers can treat the separator as invalid.
///
/// # Examples
///
/// ```rust,ignore
/// let sep_cells = vec![":--".to_string(), "---:".to_string()];
/// let formatted = mdtablefix::table::format_separator_cells(&[4, 5], &sep_cells);
///
/// assert_eq!(formatted, vec![":---".to_string(), "----:".to_string()]);
/// ```
pub(crate) fn format_separator_cells(widths: &[usize], sep_cells: &[String]) -> Vec<String> {
    if sep_cells.len() != widths.len() {
        return Vec::new();
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

/// Retain a detected separator index only while it still names an output row.
///
/// Parsing may consume or omit rows before formatting, so this guard prevents
/// a stale separator position from removing unrelated table content.
fn sep_index_within(idx: Option<usize>, len: usize) -> Option<usize> {
    match idx {
        Some(i) if i < len => Some(i),
        _ => None,
    }
}

/// Decide whether rows invalidate the table's rectangular layout invariant.
///
/// A physical-line split may legitimately create uneven intermediate rows;
/// otherwise every non-separator row must retain the first row's column count.
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

/// Matches Markdown table separator lines made only of pipes, colons, dashes,
/// and whitespace so parsing can detect and extract the alignment row.
///
/// The pattern `^[\s|:-]+$` accepts common separator forms such as
/// `| --- | :--: | --: |` while rejecting content rows that contain other
/// characters.
pub(crate) static SEP_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^[\s|:-]+$",
    "Markdown table separator row pattern should compile",
);

/// Matches a single Markdown delimiter cell: an optional leading colon, one or
/// more dashes, and an optional trailing colon.
///
/// The row-level [`SEP_RE`] cannot judge a cell on its own, because it also
/// matches an empty cell and permits whitespace inside one. A cell that merely
/// contains a dash, such as `- -`, is therefore not a delimiter cell.
pub(crate) static SEP_CELL_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^:?-+:?$",
    "Markdown table separator cell pattern should compile",
);

/// Reports whether `payload` is a single Markdown delimiter cell.
///
/// A delimiter cell is an optional colon, one or more dashes, and an optional
/// trailing colon, with no whitespace anywhere. The payload is trimmed first,
/// because the padding spaces that surround a cell in the source row are not
/// part of its content and the grammar admits no whitespace at all. Testing for
/// a dash alone was too weak: the row-level [`SEP_RE`] permits embedded
/// whitespace, so `- -` and `:- :` passed for delimiter cells and
/// [`format_separator_cells`] then rewrote them into a well-formed dash run,
/// silently turning malformed source rows into valid delimiter rows instead of
/// leaving them as data.
pub(crate) fn is_delimiter_cell(payload: &str) -> bool {
    SEP_CELL_RE.is_match(payload.trim())
}
/// Holds the parsed and validated table data.
///
/// This is produced by [`parse_and_validate`] and passed to
/// [`calculate_and_format`].
///
/// * `output_rows` - rows ready for output (separator removed)
/// * `sep_cells` - optional separator cells for formatting
/// * `max_cols` - maximum column count across all rows
struct ParsedTable {
    /// Content rows whose cell count and payload survived validation.
    output_rows: Vec<Vec<String>>,
    /// Alignment row held apart until final widths are known.
    sep_cells: Option<Vec<String>>,
    /// Widest validated row, which defines the output table's column count.
    max_cols: usize,
}

/// Extracts the leading whitespace of the first line and returns trimmed lines.
///
/// Lines beginning with `\-` are removed after trimming. These lines escape a
/// leading pipe marker and should not be part of the table.
fn extract_indent_and_trim(lines: &[String]) -> (String, Vec<String>) {
    let indent = crate::textproc::leading_indent(&lines[0]).to_string();
    let trimmed = lines
        .iter()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.trim_start().starts_with("\\-"))
        .collect();
    (indent, trimmed)
}

/// Reports whether `line` is a table delimiter row.
///
/// Every cell must be a delimiter cell as [`is_delimiter_cell`] defines it: an
/// optional colon, one or more dashes, and an optional trailing colon, the same
/// rule `reflow::row_parsing` and `reflow::second_row_is_separator` apply.
/// `SEP_RE` is only a row-level sieve, because it matches an empty cell as
/// readily as one that is dashes and permits whitespace between them, so a
/// line-level test would admit rows that merely resemble a delimiter row — and
/// [`format_separator_cells`] would then rewrite them into valid ones.
fn is_delimiter_row(line: &str) -> bool {
    SEP_RE.is_match(line) && split_cells(line).iter().all(|cell| is_delimiter_cell(cell))
}
/// Removes and returns the first delimiter row detected in `lines`.
///
/// Every cell must be a well-formed delimiter cell rather than one that merely
/// carries a dash, as it is in `reflow::row_parsing`, because `SEP_RE` alone
/// also matches a row whose cells are all empty: `|  |  |` is made only of
/// pipes and spaces, so a table whose header row is empty had that header taken
/// for the delimiter row, which demoted the real delimiter row to a data row
/// and left two delimiter-shaped rows for later passes to consume in turn. A
/// lone dash in an otherwise empty header row — `|  | - |` above
/// `| --- | --- |` — is the same trap one dash later: the header became the
/// delimiter row, the real delimiter row was laid out as text, and the
/// synthesized delimiter row grew a column wider on every pass. A cell that
/// holds both a dash and whitespace, as in the header `| - - |`, is the same
/// trap again: `SEP_RE` admits the whitespace where the cell grammar does not,
/// so the malformed header was taken for the delimiter row and rewritten into
/// `| --- |`.
fn extract_separator_line(lines: &mut Vec<String>) -> Option<String> {
    let sep_idx = lines.iter().position(|l| is_delimiter_row(l));
    sep_idx.map(|idx| lines.remove(idx))
}

/// Parses table rows and validates column consistency.
fn parse_and_validate(trimmed: &[String], sep_line: Option<&String>) -> Option<ParsedTable> {
    let (rows, split_within_line) = crate::reflow::parse_rows(trimmed);
    let max_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let (sep_cells, sep_row_idx) = crate::reflow::detect_separator(sep_line, &rows, max_cols);
    let cleaned = crate::reflow::clean_rows(rows);
    if rows_mismatched(&cleaned, split_within_line) {
        debug!(
            reason = "rows_mismatched",
            row_count = cleaned.len(),
            has_separator = sep_cells.is_some(),
            "table candidate rejected"
        );
        return None;
    }
    let mut output_rows = cleaned.clone();
    if let Some(idx) = sep_index_within(sep_row_idx, output_rows.len()) {
        output_rows.remove(idx);
    }
    // A lone single-cell candidate with no separator row is not a table: it is a
    // stray pipe-prefixed line, such as a shell pipeline continuation inside a
    // code block. Formatting it would fabricate a trailing pipe, so treat it as
    // structurally insufficient and return the input unchanged.
    if sep_cells.is_none()
        && output_rows.len() == 1
        && output_rows.first().is_some_and(|row| row.len() == 1)
    {
        debug!(
            reason = "lone_single_cell",
            row_count = output_rows.len(),
            has_separator = sep_cells.is_some(),
            "table candidate rejected"
        );
        return None;
    }
    Some(ParsedTable {
        output_rows,
        sep_cells,
        max_cols,
    })
}

/// Calculates column widths and formats the final table output.
fn calculate_and_format(parsed: &ParsedTable, indent: &str) -> Option<Vec<String>> {
    let mut widths = crate::reflow::calculate_widths(&parsed.output_rows, parsed.max_cols);
    if parsed.sep_cells.is_some() {
        for width in &mut widths {
            *width = (*width).max(3);
        }
    }
    if parsed
        .sep_cells
        .as_ref()
        .is_some_and(|cells| format_separator_cells(&widths, cells).is_empty())
    {
        return None;
    }
    let out = crate::reflow::format_rows(&parsed.output_rows, &widths, indent);
    Some(crate::reflow::insert_separator(
        out,
        parsed.sep_cells.clone(),
        &widths,
        indent,
    ))
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
    reflow_valid_table(lines).unwrap_or_else(|| lines.to_vec())
}

/// Returns whether `lines` form a table that can be reflowed.
pub(crate) fn is_valid_table(lines: &[String]) -> bool { reflow_valid_table(lines).is_some() }

/// Reflow only structurally valid table input, preserving invalid candidates.
///
/// Returning `None` is the deliberate safety boundary used by `reflow_table`
/// to leave prose, shell pipelines, and malformed tables byte-for-byte intact.
fn reflow_valid_table(lines: &[String]) -> Option<Vec<String>> {
    if lines.is_empty() {
        return Some(Vec::new());
    }

    let (indent, mut trimmed) = extract_indent_and_trim(lines);
    let sep_line = extract_separator_line(&mut trimmed);

    let parsed = parse_and_validate(&trimmed, sep_line.as_ref())?;

    calculate_and_format(&parsed, &indent)
}
#[cfg(test)]
mod tests;
