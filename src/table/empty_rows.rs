//! Reflow of generated tables whose empty rows must survive (#582).
//!
//! A source table containing a line made only of pipes is left as written,
//! because reflow would discard that line. Tables the HTML converter
//! generates still need aligning, and an empty `<tr>` becomes exactly such a
//! line, so they are reflowed without their empty rows and the empty rows are
//! put back, padded to the delimiter row's geometry.

use super::{is_pipe_only, reflow_table};

/// Reflows generated table lines, keeping each pipe-only row as an empty row.
///
/// The generated lines map one to one onto output rows, so each empty row is
/// reinserted at its original position. Its geometry is the delimiter row's
/// with every non-pipe character blanked, which pads it to the column widths.
/// Without a delimiter row, or with an empty header, there is no layout to
/// copy, and the lines are passed to [`reflow_table`] as they are.
pub(crate) fn reflow_generated_rows(lines: &[String]) -> Vec<String> {
    let (empty, filled): (Vec<_>, Vec<_>) = lines
        .iter()
        .enumerate()
        .partition(|(_, line)| is_pipe_only(line));
    // An empty header leaves no header row to lay the table out from, so the
    // lines go through unchanged, as a source table would.
    if empty.is_empty() || empty.iter().any(|(index, _)| *index < 2) {
        return reflow_table(lines);
    }
    let filled: Vec<String> = filled.into_iter().map(|(_, line)| line.clone()).collect();
    let mut out = reflow_table(&filled);
    let Some(blank) = out
        .get(1)
        .filter(|line| is_delimiter_geometry(line))
        .map(|line| blank_cells(line))
    else {
        return reflow_table(lines);
    };
    for (index, _) in empty {
        out.insert(index.min(out.len()), blank.clone());
    }
    out
}

/// Reports whether a reflowed line is a delimiter row: pipes, dashes, colons and spaces.
fn is_delimiter_geometry(line: &str) -> bool {
    line.contains('-') && line.chars().all(|ch| matches!(ch, '|' | '-' | ':' | ' '))
}

/// Replaces every character but the pipes with a space.
fn blank_cells(line: &str) -> String {
    line.chars()
        .map(|ch| if ch == '|' { ch } else { ' ' })
        .collect()
}
