//! Library for fixing markdown tables.
//!
//! Functions here reflow tables that were broken during formatting.

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
                    current.push('|');
                    chars.next();
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

    if rows.iter().any(|r| {
        let count = r.len();
        count != 0 && count != max_cols
    }) {
        return lines.to_vec();
    }

    let mut cleaned = Vec::new();
    for mut row in rows {
        row.retain(|c| !c.is_empty());
        while row.len() < max_cols {
            row.push(String::new());
        }
        cleaned.push(row);
    }

    let mut widths = vec![0; max_cols];
    for row in &cleaned {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(cell.len());
        }
    }

    cleaned
        .into_iter()
        .map(|row| {
            let padded: Vec<String> = row
                .into_iter()
                .enumerate()
                .map(|(i, c)| format!("{:<width$}", c, width = widths[i]))
                .collect();
            format!("| {} |", padded.join(" | "))
        })
        .collect();

    if let Some(sep) = sep_line {
        if let Some(first) = out.first().cloned() {
            let mut with_sep = vec![first, format!("{}{}", indent, sep)];
            with_sep.extend(out.into_iter().skip(1));
            return with_sep;
        }
        return vec![format!("{}{}", indent, sep)];
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
    let mut in_code = false;
    let mut in_table = false;

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
