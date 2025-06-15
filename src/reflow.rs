// Helper functions for reflowing markdown tables.
//
// These small utilities break down the steps of `reflow_table` so each
// piece can be understood and tested independently.

use crate::{format_separator_cells, split_cells};
use regex::Regex;

static SENTINEL_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\|\s*\|\s*").unwrap());

pub(crate) fn parse_rows(trimmed: &[String]) -> (Vec<Vec<String>>, bool) {
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
    (rows, split_within_line)
}

pub(crate) fn clean_rows(rows: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let mut cleaned = Vec::new();
    for mut row in rows {
        row.retain(|c| !c.is_empty());
        cleaned.push(row);
    }
    cleaned
}

pub(crate) fn calculate_widths(rows: &[Vec<String>], max_cols: usize) -> Vec<usize> {
    let mut widths = vec![0; max_cols];
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(cell.len());
        }
    }
    widths
}

pub(crate) fn format_rows(rows: Vec<Vec<String>>, widths: &[usize], indent: &str) -> Vec<String> {
    rows.into_iter()
        .map(|row| {
            let padded: Vec<String> = row
                .into_iter()
                .enumerate()
                .map(|(i, c)| format!("{:<width$}", c, width = widths[i]))
                .collect();
            format!("{}| {} |", indent, padded.join(" | "))
        })
        .collect()
}

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

pub(crate) fn detect_separator(
    sep_line: Option<&String>,
    rows: &[Vec<String>],
    max_cols: usize,
) -> (Option<Vec<String>>, Option<usize>) {
    let mut sep_cells: Option<Vec<String>> = sep_line.map(|l| split_cells(l));
    let mut sep_row_idx: Option<usize> = None;
    let sep_invalid = match sep_cells.as_ref() {
        Some(c) => c.len() != max_cols,
        None => true,
    };
    if sep_invalid && rows.len() > 1 && rows[1].iter().all(|c| crate::SEP_RE.is_match(c)) {
        sep_cells = Some(rows[1].clone());
        sep_row_idx = Some(1);
    }
    (sep_cells, sep_row_idx)
}
