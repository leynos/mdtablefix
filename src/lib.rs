//! Library for fixing markdown tables.
//!
//! Functions here reflow tables that were broken during formatting.
//! The [`convert_html_tables`] helper is re-exported at the crate root so
//! callers can convert simple HTML tables before reflowing.

mod html;
mod reflow;

#[doc(hidden)]
#[must_use]
pub fn html_table_to_markdown(lines: &[String]) -> Vec<String> {
    html::html_table_to_markdown(lines)
}

use std::{fs, path::Path};

pub use html::convert_html_tables;
use regex::Regex;

/// Splits a markdown table line into trimmed cell strings.
///
/// Removes leading and trailing pipe characters, splits the line by pipes, trims whitespace from
/// each cell, and returns the resulting cell strings as a vector.
///
/// # Examples
///
/// ```no_run
/// use mdtablefix::split_cells;
/// let line = "| cell1 | cell2 | cell3 |";
/// let cells = split_cells(line);
/// assert_eq!(cells, vec!["cell1", "cell2", "cell3"]);
/// ```
#[must_use]
pub fn split_cells(line: &str) -> Vec<String> {
    let mut s = line.trim();
    if let Some(stripped) = s.strip_prefix('|') {
        s = stripped;
    }
    if let Some(stripped) = s.strip_suffix('|') {
        s = stripped;
    }

    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(&next) = chars.peek()
                && next == '|'
            {
                // `\|` escapes the pipe so it becomes part of the cell
                chars.next();
                current.push('|');
                continue;
            }
            current.push(ch);
            continue;
        }
        if ch == '|' {
            cells.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    cells.push(current.trim().to_string());
    cells
}

/// Formats the cells for a separator row based on column widths.
fn format_separator_cells(widths: &[usize], sep_cells: &[String]) -> Vec<String> {
    if sep_cells.len() != widths.len() {
        // A malformed separator row could cause a panic below when indexing
        // `widths`. Return the cells unchanged so the caller can decide how to
        // handle the mismatch gracefully.
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

/// Returns the separator index if it lies within `len`.
fn sep_index_within(idx: Option<usize>, len: usize) -> Option<usize> {
    match idx {
        Some(i) if i < len => Some(i),
        _ => None,
    }
}

/// Returns `true` if rows have mismatched lengths when not split within lines.
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

/// Reflow a broken markdown table.
///
/// # Panics
/// Panics if the internal regex fails to compile.
/// Reflows a broken markdown table into properly aligned rows and columns.
///
/// Takes a slice of strings representing lines of a markdown table, reconstructs the table by
/// splitting and aligning cells, and returns the reflowed table as a vector of strings. If the rows
/// have inconsistent numbers of non-empty columns, the original lines are returned unchanged.
///
/// # Examples
///
/// ```no_run
/// use mdtablefix::reflow_table;
/// let lines = vec!["| a | b |".to_string(), "| c | d |".to_string()];
/// let fixed = reflow_table(&lines);
/// assert_eq!(
///     fixed,
///     vec!["| a | b |".to_string(), "| c | d |".to_string(),]
/// );
/// ```
pub(crate) static SEP_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^[\s|:-]+$").unwrap());

#[must_use]
pub fn reflow_table(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }

    let indent: String = lines[0].chars().take_while(|c| c.is_whitespace()).collect();
    let mut trimmed: Vec<String> = lines
        .iter()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.trim_start().starts_with("\\-"))
        .collect();
    let sep_idx = trimmed.iter().position(|l| SEP_RE.is_match(l));
    let sep_line = sep_idx.map(|idx| trimmed.remove(idx));

    let (rows, split_within_line) = reflow::parse_rows(&trimmed);

    // Count every cell, even if it is empty, to preserve column
    // positions when checking for consistency across rows.
    let max_cols = rows.iter().map(Vec::len).max().unwrap_or(0);

    let (sep_cells, sep_row_idx) = reflow::detect_separator(sep_line.as_ref(), &rows, max_cols);

    let cleaned = reflow::clean_rows(rows);

    let mut output_rows = cleaned.clone();
    if let Some(idx) = sep_index_within(sep_row_idx, output_rows.len()) {
        output_rows.remove(idx);
    }

    if rows_mismatched(&cleaned, split_within_line) {
        return lines.to_vec();
    }

    let widths = reflow::calculate_widths(&cleaned, max_cols);

    let out = reflow::format_rows(output_rows, &widths, &indent);

    reflow::insert_separator(out, sep_cells, &widths, &indent)
}

/// Processes a stream of markdown lines, reflowing tables while preserving code blocks and other
/// content.
///
/// Detects fenced code blocks and avoids modifying their contents. Buffers lines that appear to be
/// part of a markdown table and reflows them when the table ends. Non-table lines and code blocks
/// are output unchanged.
///
/// # Returns
///
/// A vector of strings representing the processed markdown document with tables reflowed.
///
/// # Examples
///
/// ```no_run
/// use mdtablefix::process_stream;
/// let input = vec![
///     "| a | b |".to_string(),
///     "|---|---|".to_string(),
///     "| 1 | 2 |".to_string(),
///     "".to_string(),
///     "```".to_string(),
///     "code block".to_string(),
///     "```".to_string(),
/// ];
/// let output = process_stream(&input);
/// assert_eq!(output[0], "| a   | b   |");
/// assert_eq!(output[1], "| --- | --- |");
/// assert_eq!(output[2], "| 1   | 2   |");
/// assert_eq!(output[3], "");
/// assert_eq!(output[4], "```");
/// assert_eq!(output[5], "code block");
/// assert_eq!(output[6], "```");
/// ```
static FENCE_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(```|~~~).*").unwrap());

static BULLET_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\s*(?:[-*+]|\d+[.)])\s+)(.*)").unwrap());

static NUMBERED_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\s*)([1-9][0-9]*)\.(\s+)(.*)").unwrap());

/// Parses a line beginning with a numbered list marker.
///
/// Returns the indentation length, separator following the number, and the
/// remainder of the line if `line` matches the numbered list pattern.
#[doc(hidden)]
fn parse_numbered(line: &str) -> Option<(usize, &str, &str)> {
    let cap = NUMBERED_RE.captures(line)?;
    let indent = cap.get(1)?.as_str().len();
    let sep = cap.get(3)?.as_str();
    let rest = cap.get(4)?.as_str();
    Some((indent, sep, rest))
}

fn tokenize_markdown(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            let start = i;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
        } else if c == '`' {
            let start = i;
            let mut delim_len = 0;
            while i < chars.len() && chars[i] == '`' {
                i += 1;
                delim_len += 1;
            }
            let mut end = i;
            while end < chars.len() {
                if chars[end] == '`' {
                    let mut j = end;
                    let mut count = 0;
                    while j < chars.len() && chars[j] == '`' {
                        j += 1;
                        count += 1;
                    }
                    if count == delim_len {
                        end = j;
                        break;
                    }
                }
                end += 1;
            }
            if end >= chars.len() {
                tokens.push(chars[start..start + delim_len].iter().collect());
                i = start + delim_len;
            } else {
                tokens.push(chars[start..end].iter().collect());
                i = end;
            }
        } else {
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '`' {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
        }
    }
    tokens
}

/// Width of a normalised thematic break.
/// The width used when rewriting thematic breaks.
pub const THEMATIC_BREAK_LEN: usize = 70;

static THEMATIC_BREAK_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"^[ ]{0,3}((?:[ \t]*\*){3,}|(?:[ \t]*-){3,}|(?:[ \t]*_){3,})[ \t]*$").unwrap()
});

static THEMATIC_BREAK_LINE: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| "_".repeat(THEMATIC_BREAK_LEN));

fn wrap_preserving_code(text: &str, width: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    for token in tokenize_markdown(text) {
        let token_width = UnicodeWidthStr::width(token.as_str());
        if current_width + token_width <= width {
            current.push_str(&token);
            current_width += token_width;
            continue;
        }

        let trimmed = current.trim_end();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
        current.clear();
        current_width = 0;

        if !token.chars().all(char::is_whitespace) {
            current.push_str(&token);
            current_width = token_width;
        }
    }
    let trimmed = current.trim_end();
    if !trimmed.is_empty() {
        lines.push(trimmed.to_string());
    }
    lines
}

/// Returns `true` if the line is a fenced code block delimiter (e.g., three backticks or "~~~").
///
/// # Examples
///
/// ```no_run
/// use mdtablefix::is_fence;
/// assert!(is_fence("```"));
/// assert!(is_fence("~~~"));
/// assert!(!is_fence("| foo | bar |"));
/// ```
#[doc(hidden)]
pub fn is_fence(line: &str) -> bool { FENCE_RE.is_match(line) }

/// Replaces spaces within inline code spans with non-breaking spaces.
///
/// Inline code spans are delimited by matching pairs of backticks. This helper
/// replaces normal spaces inside those spans with `U+00A0` (non-breaking space)
/// so that the wrapping logic does not split them across lines.
/// Flushes a buffered paragraph to the output, wrapping text to the specified width and applying
/// indentation.
///
/// Concatenates buffered lines into a single paragraph, respecting hard line breaks, and writes the
/// wrapped lines to the output vector with the given indentation. Lines are wrapped to the
/// specified width minus the indentation length. Hard breaks in the buffer force a line break at
/// that point.
fn flush_paragraph(out: &mut Vec<String>, buf: &[(String, bool)], indent: &str, width: usize) {
    if buf.is_empty() {
        return;
    }
    let mut segment = String::new();
    for (text, hard_break) in buf {
        if !segment.is_empty() {
            segment.push(' ');
        }
        segment.push_str(text);
        if *hard_break {
            for line in wrap_preserving_code(&segment, width - indent.len()) {
                out.push(format!("{indent}{line}"));
            }
            segment.clear();
        }
    }
    if !segment.is_empty() {
        for line in wrap_preserving_code(&segment, width - indent.len()) {
            out.push(format!("{indent}{line}"));
        }
    }
}

/// Wraps text lines to a specified width, preserving markdown structure.
///
/// Paragraphs and list items are reflowed to the given width, while code blocks, tables, headers,
/// and blank lines are left unchanged. Indentation and bullet/numbered list prefixes are preserved.
/// Hard line breaks (two spaces or `<br>` tags) are respected.
///
/// # Parameters
/// - `lines`: The input lines of markdown text.
/// - `width`: The maximum line width for wrapping.
///
/// # Returns
/// A vector of strings containing the wrapped and formatted markdown lines.
///
/// # Examples
///
/// ```no_run
/// use mdtablefix::wrap_text;
/// let input = vec![
///     "This is a long paragraph that should be wrapped to a shorter width.".to_string(),
///     "".to_string(),
///     "```".to_string(),
///     "let x = 42;".to_string(),
///     "```".to_string(),
/// ];
/// let wrapped = wrap_text(&input, 20);
/// assert_eq!(wrapped[0], "This is a long");
/// assert_eq!(wrapped[1], "paragraph that should");
/// assert_eq!(wrapped[2], "be wrapped to a");
/// assert_eq!(wrapped[3], "shorter width.");
/// assert_eq!(wrapped[4], "");
/// assert_eq!(wrapped[5], "```");
/// assert_eq!(wrapped[6], "let x = 42;");
/// assert_eq!(wrapped[7], "```");
/// ```
#[doc(hidden)]
pub fn wrap_text(lines: &[String], width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf: Vec<(String, bool)> = Vec::new();
    let mut indent = String::new();
    let mut in_code = false;

    for line in lines {
        if FENCE_RE.is_match(line) {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            in_code = !in_code;
            out.push(line.clone());
            continue;
        }

        if in_code {
            out.push(line.clone());
            continue;
        }

        if line.trim_start().starts_with('|') || SEP_RE.is_match(line.trim()) {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            out.push(line.clone());
            continue;
        }

        if line.trim_start().starts_with('#') {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            out.push(line.clone());
            continue;
        }

        if line.trim().is_empty() {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            out.push(String::new());
            continue;
        }

        if let Some(cap) = BULLET_RE.captures(line) {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            let prefix = cap.get(1).unwrap().as_str();
            let rest = cap.get(2).unwrap().as_str().trim();
            let spaces = " ".repeat(prefix.len());
            for (i, l) in wrap_preserving_code(rest, width - prefix.len())
                .iter()
                .enumerate()
            {
                if i == 0 {
                    out.push(format!("{prefix}{l}"));
                } else {
                    out.push(format!("{spaces}{l}"));
                }
            }
            continue;
        }

        if buf.is_empty() {
            indent = line.chars().take_while(|c| c.is_whitespace()).collect();
        }
        let trimmed_end = line.trim_end();
        let hard_break = line.ends_with("  ")
            || trimmed_end.ends_with("<br>")
            || trimmed_end.ends_with("<br/>")
            || trimmed_end.ends_with("<br />");
        let text = trimmed_end
            .trim_end_matches("<br>")
            .trim_end_matches("<br/>")
            .trim_end_matches("<br />")
            .trim_end_matches(' ')
            .trim_start()
            .to_string();
        buf.push((text, hard_break));
    }

    flush_paragraph(&mut out, &buf, &indent, width);
    out
}

#[must_use]
/// Processes a stream of markdown lines, converting HTML tables, reflowing markdown tables, and
/// wrapping text to 80 columns.
///
/// Converts simple HTML tables to markdown, reflows markdown tables for consistent alignment, and
/// wraps paragraphs and list items to 80 characters. Preserves code blocks, headers, and special
/// markdown structures.
///
/// # Returns
///
/// A vector of processed markdown lines with tables fixed and text wrapped.
///
/// # Examples
///
/// ```no_run
/// use mdtablefix::process_stream;
/// let input = vec![
///     "<table><tr><td>foo</td><td>bar</td></tr></table>".to_string(),
///     "| a | b |".to_string(),
///     "|---|---|".to_string(),
///     "| 1 | 2 |".to_string(),
///     "".to_string(),
///     "A paragraph that will be wrapped to fit within eighty columns. This sentence is \
///      intentionally long to demonstrate wrapping."
///         .to_string(),
/// ];
/// let output = process_stream(&input);
/// assert!(output.iter().any(|line| line.contains("| foo | bar |")));
/// assert!(output.iter().any(|line| line.len() <= 80));
/// ```
fn process_stream_inner(lines: &[String], wrap: bool) -> Vec<String> {
    let pre = html::convert_html_tables(lines);

    let mut out = Vec::new();
    let mut buf = Vec::new();
    let mut in_code = false;
    let mut in_table = false;

    for line in &pre {
        if FENCE_RE.is_match(line) {
            if !buf.is_empty() {
                if in_table {
                    out.extend(reflow_table(&buf));
                } else {
                    out.extend(buf.clone());
                }
                buf.clear();
            }
            in_code = !in_code;
            out.push(line.to_string());
            continue;
        }

        if in_code {
            out.push(line.to_string());
            continue;
        }

        if line.trim_start().starts_with('|') {
            if !in_table {
                in_table = true;
            }
            buf.push(line.trim_end().to_string());
            continue;
        }

        if in_table && !line.trim().is_empty() {
            buf.push(line.trim_end().to_string());
            continue;
        }

        if !buf.is_empty() {
            if in_table {
                out.extend(reflow_table(&buf));
            } else {
                out.extend(buf.clone());
            }
            buf.clear();
            in_table = false;
        }

        out.push(line.to_string());
    }

    if !buf.is_empty() {
        if in_table {
            out.extend(reflow_table(&buf));
        } else {
            out.extend(buf);
        }
    }

    if wrap { wrap_text(&out, 80) } else { out }
}

#[must_use]
/// Renumbers ordered list items in Markdown text.
///
/// Lines matching `^\s*[1-9][0-9]*\.\s+` are renumbered sequentially within
/// their indentation level. Numbering continues across fenced code blocks
/// without resetting.
pub fn renumber_lists(lines: &[String]) -> Vec<String> {
    use std::collections::HashMap;

    let mut out = Vec::with_capacity(lines.len());
    let mut stack = Vec::<usize>::new();
    let mut counters = HashMap::<usize, usize>::new();
    let mut in_code = false;

    for line in lines {
        if FENCE_RE.is_match(line) {
            in_code = !in_code;
            out.push(line.clone());
            continue;
        }

        if in_code {
            out.push(line.clone());
            continue;
        }

        if let Some((indent, sep, rest)) = parse_numbered(line) {
            while stack.last().is_some_and(|&d| d > indent) {
                if let Some(d) = stack.pop() {
                    counters.remove(&d);
                }
            }

            if stack.last().is_none_or(|&d| d < indent) {
                stack.push(indent);
            }

            let num = counters.entry(indent).or_insert(1);
            let current = *num;
            *num += 1;
            out.push(format!("{}{}.{}{}", " ".repeat(indent), current, sep, rest));
            continue;
        }

        let indent = line.chars().take_while(|c| c.is_whitespace()).count();
        while stack.last().is_some_and(|&d| d > indent) {
            if let Some(d) = stack.pop() {
                counters.remove(&d);
            }
        }
        out.push(line.clone());
    }

    out
}

#[must_use]
/// Reformat thematic breaks as 70 underscores.
///
/// Thematic breaks are lines composed of three or more matching `-`, `_`, or
/// `*` characters (optionally separated by spaces or tabs) with up to three
/// leading spaces. Lines inside fenced code blocks are ignored.
pub fn format_breaks(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut in_code = false;

    for line in lines {
        if FENCE_RE.is_match(line) {
            in_code = !in_code;
            out.push(line.clone());
            continue;
        }

        if !in_code && THEMATIC_BREAK_RE.is_match(line.trim_end()) {
            out.push(THEMATIC_BREAK_LINE.clone());
        } else {
            out.push(line.clone());
        }
    }

    out
}

#[must_use]
pub fn process_stream(lines: &[String]) -> Vec<String> { process_stream_inner(lines, true) }

#[must_use]
pub fn process_stream_no_wrap(lines: &[String]) -> Vec<String> {
    process_stream_inner(lines, false)
}

/// Rewrite a file in place with fixed tables.
///
/// # Errors
/// Reads a markdown file, reflows any broken tables within it, and writes the updated content back
/// to the same file.
///
/// Returns an error if the file cannot be read or written.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
///
/// use mdtablefix::rewrite;
/// let path = Path::new("example.md");
/// rewrite(path).unwrap();
/// ```
pub fn rewrite(path: &Path) -> std::io::Result<()> {
    let text = fs::read_to_string(path)?;
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let fixed = process_stream(&lines);
    fs::write(path, fixed.join("\n") + "\n")
}

/// Rewrite a file in place with fixed tables without wrapping text.
///
/// # Errors
/// Returns an error if the file cannot be read or written.
pub fn rewrite_no_wrap(path: &Path) -> std::io::Result<()> {
    let text = fs::read_to_string(path)?;
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let fixed = process_stream_no_wrap(&lines);
    fs::write(path, fixed.join("\n") + "\n")
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

    #[test]
    fn wrap_text_preserves_hyphenated_words() {
        let input = vec!["A word that is very-long-word indeed".to_string()];
        let wrapped = wrap_text(&input, 20);
        assert_eq!(
            wrapped,
            vec![
                "A word that is".to_string(),
                "very-long-word".to_string(),
                "indeed".to_string(),
            ]
        );
    }

    #[test]
    fn wrap_text_preserves_code_spans() {
        let input = vec![
            "with their own escaping rules. On Windows, scripts default to `powershell -Command` \
             unless the manifest's `interpreter` field overrides the setting."
                .to_string(),
        ];
        let wrapped = wrap_text(&input, 60);
        assert_eq!(
            wrapped,
            vec![
                "with their own escaping rules. On Windows, scripts default".to_string(),
                "to `powershell -Command` unless the manifest's `interpreter`".to_string(),
                "field overrides the setting.".to_string(),
            ]
        );
    }

    #[test]
    fn wrap_text_multiple_code_spans() {
        let input = vec!["combine `foo bar` and `baz qux` in one line".to_string()];
        let wrapped = wrap_text(&input, 25);
        assert_eq!(
            wrapped,
            vec![
                "combine `foo bar` and".to_string(),
                "`baz qux` in one line".to_string(),
            ]
        );
    }

    #[test]
    fn wrap_text_nested_backticks() {
        let input = vec!["Use `` `code` `` to quote backticks".to_string()];
        let wrapped = wrap_text(&input, 20);
        assert_eq!(
            wrapped,
            vec![
                "Use `` `code` `` to".to_string(),
                "quote backticks".to_string()
            ]
        );
    }

    #[test]
    fn wrap_text_unmatched_backticks() {
        let input = vec!["This has a `dangling code span.".to_string()];
        let wrapped = wrap_text(&input, 20);
        assert_eq!(
            wrapped,
            vec!["This has a `dangling".to_string(), "code span.".to_string()]
        );
    }
}
