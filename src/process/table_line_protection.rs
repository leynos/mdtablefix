//! Preserves repaired table lines while non-table code-emphasis is processed.
//!
//! Table cells receive code-emphasis repair before reflow, so their final text
//! must not enter the later non-table repair pass. This module replaces table
//! lines with collision-resistant placeholders after heading conversion, then
//! restores them afterwards.

use std::collections::HashMap;

pub(super) struct ProtectedTableLines {
    replacements: HashMap<String, String>,
}

pub(super) fn protect_table_lines(
    mut lines: Vec<String>,
    table_lines: &[String],
) -> (Vec<String>, ProtectedTableLines) {
    let mut nonce = 0_usize;
    let placeholder_prefix = loop {
        let candidate = format!("\u{E000}mdtablefix-table-{nonce}\u{E001}");
        if lines.iter().all(|line| !line.contains(&candidate)) {
            break candidate;
        }
        nonce += 1;
    };

    let mut table_line_counts: HashMap<&str, usize> = HashMap::new();
    for table_line in table_lines {
        *table_line_counts
            .entry(table_line.as_str())
            .or_insert(0_usize) += 1;
    }

    let mut replacements = HashMap::new();
    for (index, line) in lines.iter_mut().enumerate() {
        if let Some(remaining) = table_line_counts.get_mut(line.as_str()) {
            if *remaining == 0 {
                continue;
            }
            *remaining -= 1;
            let placeholder = format!("{placeholder_prefix}{index}");
            let table_line = std::mem::replace(line, placeholder.clone());
            replacements.insert(placeholder, table_line);
        }
    }

    (lines, ProtectedTableLines { replacements })
}

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
