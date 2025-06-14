//! Library for fixing markdown tables.
//!
//! Functions here reflow tables that were broken during formatting.

use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use regex::Regex;
use std::fs;
use std::path::Path;

/// Splits a markdown table line into trimmed cell strings.
///
/// Removes leading and trailing pipe characters, splits the line by pipes, trims whitespace from each cell, and returns the resulting cell strings as a vector.
///
/// # Examples
///
/// ```ignore
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
            if let Some(&next) = chars.peek() {
                if next == '|' {
                    // `\|` escapes the pipe so it becomes part of the cell
                    chars.next();
                    current.push('|');
                    continue;
                }
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

fn node_text(handle: &Handle) -> String {
    let mut parts = Vec::new();
    collect_text(handle, &mut parts);
    parts
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn collect_text(handle: &Handle, out: &mut Vec<String>) {
    match &handle.data {
        NodeData::Text { contents } => out.push(contents.borrow().to_string()),
        NodeData::Element { name, .. } => {
            let tag = name.local.as_ref();
            if tag.eq_ignore_ascii_case("script")
                || tag.eq_ignore_ascii_case("style")
                || tag.eq_ignore_ascii_case("noscript")
                || tag.eq_ignore_ascii_case("template")
                || tag.eq_ignore_ascii_case("head")
            {
                return;
            }
            for child in handle.children.borrow().iter() {
                collect_text(child, out);
            }
        }
        NodeData::Document => {
            for child in handle.children.borrow().iter() {
                collect_text(child, out);
            }
        }
        _ => {}
    }
}

fn collect_tables(handle: &Handle, tables: &mut Vec<Handle>) {
    if let NodeData::Element { name, .. } = &handle.data {
        if name.local.as_ref() == "table" {
            tables.push(handle.clone());
        }
    }
    for child in handle.children.borrow().iter() {
        collect_tables(child, tables);
    }
}

fn collect_rows(handle: &Handle, rows: &mut Vec<Handle>) {
    if let NodeData::Element { name, .. } = &handle.data {
        if name.local.as_ref() == "tr" {
            rows.push(handle.clone());
        }
    }
    for child in handle.children.borrow().iter() {
        collect_rows(child, rows);
    }
}

use html5ever::driver::ParseOpts;

fn table_node_to_markdown(table: &Handle) -> Vec<String> {
    let mut row_handles = Vec::new();
    collect_rows(table, &mut row_handles);
    if row_handles.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut first_header = false;
    let mut col_count = 0;
    for (i, row) in row_handles.iter().enumerate() {
        let mut cells = Vec::new();
        let mut header_row = false;
        for child in row.children.borrow().iter() {
            if let NodeData::Element { name, .. } = &child.data {
                if name.local.as_ref() == "td" || name.local.as_ref() == "th" {
                    if name.local.as_ref() == "th" {
                        header_row = true;
                    }
                    cells.push(node_text(child));
                }
            }
        }
        if i == 0 {
            first_header = header_row;
            col_count = cells.len();
        }
        out.push(format!("| {} |", cells.join(" | ")));
    }
    if first_header {
        let sep: Vec<String> = (0..col_count).map(|_| "---".to_string()).collect();
        out.insert(1, format!("| {} |", sep.join(" | ")));
    }
    reflow_table(&out)
}

fn html_table_to_markdown(lines: &[String]) -> Vec<String> {
    let indent: String = lines
        .first()
        .map(|l| l.chars().take_while(|c| c.is_whitespace()).collect())
        .unwrap_or_default();
    let html: String = lines
        .iter()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    let opts = ParseOpts::default();
    let dom: RcDom = parse_document(RcDom::default(), opts).one(html);

    let mut tables = Vec::new();
    collect_tables(&dom.document, &mut tables);
    if tables.is_empty() {
        return lines.to_vec();
    }

    let mut out = Vec::new();
    for table in tables {
        for line in table_node_to_markdown(&table) {
            out.push(format!("{indent}{line}"));
        }
    }
    out
}

/// Reflow a broken markdown table.
///
/// # Panics
/// Panics if the internal regex fails to compile.
/// Reflows a broken markdown table into properly aligned rows and columns.
///
/// Takes a slice of strings representing lines of a markdown table, reconstructs the table by splitting and aligning cells, and returns the reflowed table as a vector of strings. If the rows have inconsistent numbers of non-empty columns, the original lines are returned unchanged.
///
/// # Examples
///
/// ```ignore
/// use mdtablefix::reflow_table;
/// let lines = vec![
///     "| a | b |".to_string(),
///     "| c | d |".to_string(),
/// ];
/// let fixed = reflow_table(&lines);
/// assert_eq!(fixed, vec![
///     "| a | b |".to_string(),
///     "| c | d |".to_string(),
/// ]);
/// ```
static SENTINEL_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\|\s*\|\s*").unwrap());
static SEP_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^[\s|:-]+$").unwrap());

#[must_use]
pub fn reflow_table(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }

    let indent: String = lines[0].chars().take_while(|c| c.is_whitespace()).collect();
    let mut trimmed: Vec<String> = lines.iter().map(|l| l.trim().to_string()).collect();
    let sep_idx = trimmed.iter().position(|l| SEP_RE.is_match(l));
    let sep_line = sep_idx.map(|idx| trimmed.remove(idx));

    let raw = trimmed.join(" ");
    let chunks: Vec<&str> = SENTINEL_RE.split(&raw).collect();
    let split_within_line = chunks.len() > trimmed.len();
    let mut cells = Vec::new();
    for (idx, chunk) in chunks.iter().enumerate() {
        let mut ch = (*chunk).to_string();
        if idx != chunks.len() - 1 {
            ch = ch.trim_end().to_string() + " |ROW_END|";
        }
        cells.extend(split_cells(&ch));
    }

    let mut rows = Vec::new();
    let mut current = Vec::new();
    for cell in cells {
        if cell == "ROW_END" {
            if !current.is_empty() {
                rows.push(current);
                current = Vec::new();
            }
        } else {
            current.push(cell);
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }

    // Count every cell, even if it is empty, to preserve column
    // positions when checking for consistency across rows.
    let max_cols = rows.iter().map(Vec::len).max().unwrap_or(0);

    let mut cleaned = Vec::new();
    for mut row in rows {
        row.retain(|c| !c.is_empty());
        cleaned.push(row);
    }

    if !split_within_line {
        if let Some(first_len) = cleaned.first().map(Vec::len) {
            if cleaned[1..].iter().any(|row| row.len() != first_len) {
                return lines.to_vec();
            }
        }
    }

    let mut widths = vec![0; max_cols];
    for row in &cleaned {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(cell.len());
        }
    }

    let out: Vec<String> = cleaned
        .into_iter()
        .map(|row| {
            let padded: Vec<String> = row
                .into_iter()
                .enumerate()
                .map(|(i, c)| format!("{:<width$}", c, width = widths[i]))
                .collect();
            format!("{}| {} |", indent, padded.join(" | "))
        })
        .collect();

    if let Some(sep) = sep_line {
        let mut sep_cells: Vec<String> = split_cells(&sep);
        while sep_cells.len() < widths.len() {
            sep_cells.push(String::new());
        }
        let sep_padded = format_separator_cells(&widths, &sep_cells);
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

/// Processes a stream of markdown lines, reflowing tables while preserving code blocks and other content.
///
/// Detects fenced code blocks and avoids modifying their contents. Buffers lines that appear to be part of a markdown table and reflows them when the table ends. Non-table lines and code blocks are output unchanged.
///
/// # Returns
///
/// A vector of strings representing the processed markdown document with tables reflowed.
///
/// # Examples
///
/// ```ignore
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

#[must_use]
pub fn process_stream(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = Vec::new();
    let mut html_buf = Vec::new();
    let mut html_depth = 0usize;
    let mut in_code = false;
    let mut in_table = false;
    let mut in_html = false;

    for line in lines {
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
            out.push(line.trim_end().to_string());
            continue;
        }

        if in_code {
            out.push(line.trim_end().to_string());
            continue;
        }

        if in_html {
            html_buf.push(line.trim_end().to_string());
            html_depth += line.matches("<table").count();
            if line.contains("</table>") {
                html_depth = html_depth.saturating_sub(line.matches("</table>").count());
                if html_depth == 0 {
                    out.extend(html_table_to_markdown(&html_buf));
                    html_buf.clear();
                    in_html = false;
                }
            }
            continue;
        }

        if line.trim_start().starts_with("<table") {
            if !buf.is_empty() {
                if in_table {
                    out.extend(reflow_table(&buf));
                } else {
                    out.extend(buf.clone());
                }
                buf.clear();
                in_table = false;
            }
            in_html = true;
            html_buf.push(line.trim_end().to_string());
            html_depth = line.matches("<table").count();
            if line.contains("</table>") {
                html_depth = html_depth.saturating_sub(line.matches("</table>").count());
                if html_depth == 0 {
                    out.extend(html_table_to_markdown(&html_buf));
                    html_buf.clear();
                    in_html = false;
                }
            }
            continue;
        }

        if line.trim_start().starts_with('|') {
            if !in_table {
                in_table = true;
            }
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
        out.push(line.trim_end().to_string());
    }

    if !buf.is_empty() {
        if in_table {
            out.extend(reflow_table(&buf));
        } else {
            out.extend(buf);
        }
    }

    if !html_buf.is_empty() {
        out.extend(html_table_to_markdown(&html_buf));
    }

    out
}

/// Rewrite a file in place with fixed tables.
///
/// # Errors
/// Reads a markdown file, reflows any broken tables within it, and writes the updated content back to the same file.
///
/// Returns an error if the file cannot be read or written.
///
/// # Examples
///
/// ```ignore
/// use std::path::Path;
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
