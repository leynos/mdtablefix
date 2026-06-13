//! Reordering of footnote-definition blocks for footnote renumbering.
//!
//! The sibling [`definitions`](super::definitions) module owns definition
//! scanning; this module takes the resulting rewrite plan and reorders the
//! definition block so definitions appear in ascending footnote-number order.
//! Splitting the two keeps each source file readable and within the
//! repository size limit.

use std::collections::HashMap;

use tracing::{debug, warn};

use super::{
    definitions::{DefinitionLine, definition_segment_end},
    is_definition_continuation,
    parse_definition,
};

/// One reorderable unit: `(new_number, original_index, lines)`.
///
/// `new_number` drives the sort order, `original_index` breaks ties, and
/// `lines` holds the definition header plus any leading and continuation rows
/// that must travel with it.
type DefinitionSegment = (usize, usize, Vec<String>);

fn collect_header_positions(lines: &[String], start: usize, end: usize) -> Vec<usize> {
    (start..end)
        .filter(|&idx| parse_definition(&lines[idx]).is_some())
        .collect()
}

fn build_def_lookup(
    definitions: &[DefinitionLine],
    start: usize,
    end: usize,
) -> HashMap<usize, &DefinitionLine> {
    definitions
        .iter()
        .filter(|definition| (start..end).contains(&definition.index))
        .map(|definition| (definition.index, definition))
        .collect()
}

fn leading_segment_start(lines: &[String], consumed: usize, position: usize) -> usize {
    let mut leading_start = position;
    while leading_start > consumed
        && lines[leading_start - 1].trim().is_empty()
        && !is_definition_continuation(&lines[leading_start - 1])
    {
        leading_start -= 1;
    }
    leading_start
}

fn build_definition_segment(
    lines: &[String],
    definition: &DefinitionLine,
    leading_start: usize,
    position: usize,
    next_bound: usize,
) -> DefinitionSegment {
    let mut segment = Vec::with_capacity(next_bound.saturating_sub(leading_start).max(1));
    segment.extend(lines[leading_start..position].iter().cloned());
    segment.push(definition.line.clone());
    let tail_start = position.saturating_add(1);
    if tail_start < next_bound {
        segment.extend(lines[tail_start..next_bound].iter().cloned());
    }
    (definition.new_number, definition.index, segment)
}

fn build_segments(
    lines: &[String],
    header_positions: &[usize],
    def_lookup: &HashMap<usize, &DefinitionLine>,
    start: usize,
    end: usize,
) -> Vec<DefinitionSegment> {
    let prefix_len = header_positions.first().map_or(0, |first| first - start);
    let mut consumed = start + prefix_len;
    let mut segments = Vec::new();

    for &position in header_positions {
        let leading_start = leading_segment_start(lines, consumed, position);
        let next_bound = definition_segment_end(lines, position, end);
        if let Some(definition) = def_lookup.get(&position) {
            debug_assert!(
                position >= leading_start,
                "definition header {position} cannot precede leading segment start {leading_start}",
            );
            segments.push(build_definition_segment(
                lines,
                definition,
                leading_start,
                position,
                next_bound,
            ));
        }
        consumed = next_bound;
    }

    segments
}

fn migrate_first_leading(segments: &mut [DefinitionSegment]) -> Vec<String> {
    if let Some((_, _, first_segment)) = segments.first_mut() {
        let first_content = first_segment
            .iter()
            .position(|line| !line.trim().is_empty() || is_definition_continuation(line))
            .unwrap_or(first_segment.len());
        return first_segment.drain(..first_content).collect();
    }
    Vec::new()
}

fn compose_reordered_block(
    lines: &[String],
    start: usize,
    prefix_len: usize,
    segments: Vec<DefinitionSegment>,
    mut first_leading: Vec<String>,
) -> Vec<String> {
    let mut reordered = Vec::new();
    if prefix_len > 0 {
        reordered.extend(lines[start..start + prefix_len].iter().cloned());
    }

    for (idx, (_, _, segment)) in segments.into_iter().enumerate() {
        reordered.extend(segment);
        if idx == 0 && !first_leading.is_empty() {
            reordered.append(&mut first_leading);
        }
    }

    reordered
}

/// Reorders the definition block in `lines[start..end]` so its definitions
/// appear in ascending `new_number` order, ties broken by original `index`.
///
/// `definitions` supplies the new numbering. Continuation lines stay
/// attached to their definition, the block prefix (any rows before the
/// first definition) is preserved, and leading blank lines on the first
/// reordered segment are migrated to the boundary between the first and
/// second segments so block-level spacing is not lost. The slice is mutated
/// in place; if reordering would change row count, a warning is emitted and
/// the reorder is skipped.
pub(super) fn reorder_definition_block(
    lines: &mut [String],
    start: usize,
    end: usize,
    definitions: &[DefinitionLine],
) {
    let header_positions = collect_header_positions(lines, start, end);
    if header_positions.len() <= 1 {
        debug!(
            start,
            end,
            header_count = header_positions.len(),
            "reorder_definition_block: skipping reorder without multiple headers"
        );
        return;
    }

    let def_lookup = build_def_lookup(definitions, start, end);
    if def_lookup.len() <= 1 {
        debug!(
            start,
            end,
            definition_count = def_lookup.len(),
            "reorder_definition_block: skipping reorder without multiple definition mappings"
        );
        return;
    }

    let prefix_len = header_positions.first().map_or(0, |first| first - start);
    let mut segments = build_segments(lines, &header_positions, &def_lookup, start, end);
    if segments.len() <= 1 {
        debug!(
            start,
            end,
            segment_count = segments.len(),
            "reorder_definition_block: skipping reorder without multiple segments"
        );
        return;
    }

    segments.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    let first_leading = migrate_first_leading(&mut segments);
    let reordered = compose_reordered_block(lines, start, prefix_len, segments, first_leading);

    if reordered.len() == end - start {
        for (target, source) in lines[start..end].iter_mut().zip(reordered) {
            *target = source;
        }
    } else {
        warn!(
            expected = end - start,
            actual = reordered.len(),
            "reorder_definition_block: segment count mismatch; skipping reorder",
        );
    }
}

#[cfg(test)]
mod tests;
