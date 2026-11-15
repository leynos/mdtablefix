//! Preserves repaired table lines while non-table code-emphasis is processed.
//!
//! Table cells receive code-emphasis repair before reflow, so their final text
//! must not enter the later non-table repair pass. This module replaces table
//! lines with collision-resistant placeholders after heading conversion, then
//! restores them afterwards.

use std::collections::HashMap;

/// Placeholder mapping that keeps already reflowed table rows out of later
/// non-table transformations.
pub(super) struct ProtectedTableLines {
    /// Collision-resistant placeholder to original table-line mapping.
    replacements: HashMap<String, String>,
}

/// Replaces table rows with unique placeholders before a later text pass.
///
/// Duplicate table rows are consumed in source order, and the generated
/// placeholder prefix is selected not to occur in the existing text.
pub(super) fn protect_table_lines(
    mut lines: Vec<String>,
    table_lines: &[String],
) -> (Vec<String>, ProtectedTableLines) {
    let placeholder_prefix = unused_placeholder_prefix(&lines);
    let mut table_line_counts = count_table_lines(table_lines);
    let replacements = replace_table_lines(&mut lines, &mut table_line_counts, &placeholder_prefix);
    (lines, ProtectedTableLines { replacements })
}

/// Selects a placeholder prefix that occurs in none of `lines`.
fn unused_placeholder_prefix(lines: &[String]) -> String {
    let mut nonce = 0_usize;
    loop {
        let candidate = format!("\u{E000}mdtablefix-table-{nonce}\u{E001}");
        if lines.iter().all(|line| !line.contains(&candidate)) {
            return candidate;
        }
        nonce += 1;
    }
}

/// Counts how many times each distinct table row occurs.
fn count_table_lines(table_lines: &[String]) -> HashMap<&str, usize> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for table_line in table_lines {
        *counts.entry(table_line.as_str()).or_insert(0_usize) += 1;
    }
    counts
}

/// Swaps each remaining table row in `lines` for a placeholder and returns the originals.
///
/// Each swap consumes one count, so duplicate rows are replaced in source order.
fn replace_table_lines(
    lines: &mut [String],
    counts: &mut HashMap<&str, usize>,
    placeholder_prefix: &str,
) -> HashMap<String, String> {
    let mut replacements = HashMap::new();
    for (index, line) in lines.iter_mut().enumerate() {
        let Some(remaining) = counts.get_mut(line.as_str()) else {
            continue;
        };
        if *remaining == 0 {
            continue;
        }
        *remaining -= 1;
        let placeholder = format!("{placeholder_prefix}{index}");
        let table_line = std::mem::replace(line, placeholder.clone());
        replacements.insert(placeholder, table_line);
    }
    replacements
}

/// Restores protected table rows after non-table transformations finish.
///
/// Lines without a placeholder are passed through unchanged, which allows the
/// surrounding processing stages to add or remove ordinary rows safely.
pub(super) fn restore_table_lines(
    lines: Vec<String>,
    protected_table_lines: &ProtectedTableLines,
) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| {
            protected_table_lines
                .replacements
                .get(&line)
                .cloned()
                .unwrap_or(line)
        })
        .collect()
}
