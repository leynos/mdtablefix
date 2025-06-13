//! Library for fixing markdown tables.
//!
//! Functions here reflow tables that were broken during formatting.

use regex::Regex;
use std::fs;
use std::path::Path;

/// Split a markdown table line into its cells.
#[must_use]
fn split_cells(line: &str) -> Vec<String> {
    let mut s = line.trim();
    if let Some(stripped) = s.strip_prefix('|') {
        s = stripped;
    }
    if let Some(stripped) = s.strip_suffix('|') {
        s = stripped;
    }
    s.split('|').map(|c| c.trim().to_string()).collect()
}

/// Reflow a broken markdown table.
///
/// # Panics
/// Panics if the internal regex fails to compile.
#[must_use]
pub fn reflow_table(lines: &[String]) -> Vec<String> {
    let raw = lines.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ");
    let sentinel_re = Regex::new(r"\|\s*\|\s*").unwrap();
    let chunks: Vec<&str> = sentinel_re.split(&raw).collect();
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

    let max_cols = rows
        .iter()
        .map(|r| r.iter().filter(|c| !c.is_empty()).count())
        .max()
        .unwrap_or(0);

    if rows.iter().any(|r| {
        let count = r.iter().filter(|c| !c.is_empty()).count();
        count != 0 && count != max_cols
    }) {
        return lines.to_vec();
    }

    rows.into_iter()
        .map(|mut r| {
            r.retain(|c| !c.is_empty());
            while r.len() < max_cols {
                r.push(String::new());
            }
            format!("| {} |", r.join(" | "))
        })
        .collect()
}

/// Process a stream of markdown lines, reflowing tables.
///
/// # Panics
/// Panics if the regex used for code fences fails to compile.
#[must_use]
pub fn process_stream(lines: &[String]) -> Vec<String> {
    let fence_re = Regex::new(r"^(```|~~~)").unwrap();
    let mut out = Vec::new();
    let mut buf = Vec::new();
    let mut in_code = false;
    let mut in_table = false;

    for line in lines {
        if fence_re.is_match(line) {
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
/// Returns an error if the file cannot be read or written.
pub fn rewrite(path: &Path) -> std::io::Result<()> {
    let text = fs::read_to_string(path)?;
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let fixed = process_stream(&lines);
    fs::write(path, fixed.join("\n") + "\n")
}
