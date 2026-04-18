//! Definition scanning and reordering helpers for footnote renumbering.
//!
//! The parent module keeps the top-level rewrite flow, while this submodule
//! owns the detail-heavy definition parsing and reordering machinery so each
//! source file stays readable and within the repository size limit.

use std::collections::HashMap;

use super::{
    FOOTNOTE_LINE_RE,
    footnote_block_range,
    has_existing_footnote_block,
    is_definition_continuation,
    is_fence_line,
    parse_definition,
    rewrite_tokens,
};

#[derive(Clone)]
pub(super) struct DefinitionLine {
    pub(super) index: usize,
    pub(super) new_number: usize,
    pub(super) line: String,
}

pub(super) struct NumericCandidate {
    index: usize,
    number: usize,
    indent: String,
    whitespace: String,
    rest: String,
}

pub(super) struct DefinitionUpdates {
    pub(super) definitions: Vec<DefinitionLine>,
    pub(super) is_definition_line: Vec<bool>,
}

struct DefinitionScanState<'a> {
    mapping: &'a mut HashMap<usize, usize>,
    next_number: &'a mut usize,
    numeric_list_range: Option<(usize, usize)>,
    skip_numeric_conversion: bool,
    definitions: Vec<DefinitionLine>,
    is_definition_line: Vec<bool>,
    numeric_candidates: Vec<NumericCandidate>,
}

fn definition_segment_end(lines: &[String], start: usize, block_end: usize) -> usize {
    let mut idx = start + 1;
    while idx < block_end {
        let line = &lines[idx];
        if parse_definition(line).is_some() {
            break;
        }
        if is_definition_continuation(line) {
            idx += 1;
            continue;
        }
        if line.trim().is_empty() {
            if idx + 1 < block_end && parse_definition(&lines[idx + 1]).is_some() {
                break;
            }
            idx += 1;
            continue;
        }
        break;
    }
    idx
}

fn assign_new_number(
    mapping: &mut HashMap<usize, usize>,
    number: usize,
    next_number: &mut usize,
) -> usize {
    if let Some(&mapped) = mapping.get(&number) {
        mapped
    } else {
        let assigned = *next_number;
        *next_number += 1;
        mapping.insert(number, assigned);
        assigned
    }
}

fn should_convert_numeric_line(
    index: usize,
    numeric_range: Option<(usize, usize)>,
    skip_numeric_conversion: bool,
) -> bool {
    if skip_numeric_conversion {
        return false;
    }
    numeric_range.is_some_and(|(start, end)| index >= start && index < end)
}

fn definition_line_from_parts(
    index: usize,
    parts: super::super::parsing::DefinitionParts<'_>,
    mapping: &mut HashMap<usize, usize>,
    next_number: &mut usize,
) -> DefinitionLine {
    let new_number = assign_new_number(mapping, parts.number, next_number);
    let rewritten_rest = rewrite_tokens(parts.rest, mapping);
    let mut line = String::with_capacity(parts.prefix.len() + rewritten_rest.len() + 8);
    line.push_str(parts.prefix);
    let header = format!("[^{new_number}]:");
    line.push_str(&header);
    line.push_str(&rewritten_rest);
    DefinitionLine {
        index,
        new_number,
        line,
    }
}

pub(super) fn numeric_candidate_from_line(line: &str, index: usize) -> Option<NumericCandidate> {
    let caps = FOOTNOTE_LINE_RE.captures(line)?;
    let indent = caps.name("indent").map_or("", |m| m.as_str()).to_string();
    let num_match = caps.name("num")?;
    let rest_match = caps.name("rest")?;
    let number = num_match.as_str().parse::<usize>().ok()?;
    let rest = rest_match.as_str().to_string();
    let whitespace = line[num_match.end() + 1..rest_match.start()].to_string();
    Some(NumericCandidate {
        index,
        number,
        indent,
        whitespace,
        rest,
    })
}

fn collect_scan_updates(lines: &[String], state: &mut DefinitionScanState<'_>) {
    let mut in_fence = false;

    for (index, line) in lines.iter().enumerate() {
        if is_fence_line(line) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        if let Some(parts) = parse_definition(line) {
            state.definitions.push(definition_line_from_parts(
                index,
                parts,
                state.mapping,
                state.next_number,
            ));
            state.is_definition_line[index] = true;
            continue;
        }

        if !should_convert_numeric_line(
            index,
            state.numeric_list_range,
            state.skip_numeric_conversion,
        ) {
            continue;
        }
        if state.mapping.is_empty() && state.definitions.is_empty() {
            continue;
        }
        if let Some(candidate) = numeric_candidate_from_line(line, index) {
            state.numeric_candidates.push(candidate);
        }
    }
}

fn finalize_numeric_candidates(state: &mut DefinitionScanState<'_>) {
    // Drain from the bottom so wrapped continuation lines stay attached to the
    // correct definition when numeric candidates are later reordered by their
    // assigned footnote numbers.
    for candidate in state.numeric_candidates.drain(..).rev() {
        let new_number = assign_new_number(state.mapping, candidate.number, state.next_number);
        let rewritten_rest = rewrite_tokens(&candidate.rest, state.mapping);
        let mut line = String::with_capacity(
            candidate.indent.len() + candidate.whitespace.len() + rewritten_rest.len() + 8,
        );
        line.push_str(&candidate.indent);
        let header = format!("[^{new_number}]:");
        line.push_str(&header);
        line.push_str(&candidate.whitespace);
        line.push_str(&rewritten_rest);
        state.definitions.push(DefinitionLine {
            index: candidate.index,
            new_number,
            line,
        });
        state.is_definition_line[candidate.index] = true;
    }
}

pub(super) fn collect_definition_updates(
    lines: &[String],
    mapping: &mut HashMap<usize, usize>,
) -> DefinitionUpdates {
    let mut next_number = mapping.values().copied().max().unwrap_or(0) + 1;
    let numeric_list_range = footnote_block_range(lines);
    let skip_numeric_conversion = numeric_list_range
        .as_ref()
        .is_some_and(|(start, _)| has_existing_footnote_block(lines, *start));
    let mut state = DefinitionScanState {
        mapping,
        next_number: &mut next_number,
        numeric_list_range,
        skip_numeric_conversion,
        definitions: Vec::new(),
        is_definition_line: vec![false; lines.len()],
        numeric_candidates: Vec::new(),
    };
    collect_scan_updates(lines, &mut state);
    finalize_numeric_candidates(&mut state);

    DefinitionUpdates {
        definitions: state.definitions,
        is_definition_line: state.is_definition_line,
    }
}

pub(super) fn rewrite_definition_headers(lines: &mut [String], definitions: &[DefinitionLine]) {
    for definition in definitions {
        lines[definition.index].clone_from(&definition.line);
    }
}

pub(super) fn reorder_definition_block(
    lines: &mut [String],
    start: usize,
    end: usize,
    definitions: &[DefinitionLine],
) {
    let header_positions: Vec<usize> = (start..end)
        .filter(|&idx| parse_definition(&lines[idx]).is_some())
        .collect();
    if header_positions.len() <= 1 {
        return;
    }

    let def_lookup: HashMap<usize, &DefinitionLine> = definitions
        .iter()
        .filter(|definition| (start..end).contains(&definition.index))
        .map(|definition| (definition.index, definition))
        .collect();
    if def_lookup.len() <= 1 {
        return;
    }

    let prefix_len = header_positions.first().map_or(0, |first| first - start);
    let mut segments: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut consumed = start + prefix_len;
    for &position in &header_positions {
        let mut leading_start = position;
        while leading_start > consumed
            && lines[leading_start - 1].trim().is_empty()
            && !is_definition_continuation(&lines[leading_start - 1])
        {
            leading_start -= 1;
        }
        let next_bound = definition_segment_end(lines, position, end);
        if let Some(definition) = def_lookup.get(&position) {
            debug_assert!(
                position >= leading_start,
                "definition header {position} cannot precede leading segment start {leading_start}",
            );
            let mut segment = Vec::with_capacity(next_bound.saturating_sub(leading_start).max(1));
            segment.extend(lines[leading_start..position].iter().cloned());
            segment.push(definition.line.clone());
            let tail_start = position.saturating_add(1);
            if tail_start < next_bound {
                segment.extend(lines[tail_start..next_bound].iter().cloned());
            }
            segments.push((definition.new_number, definition.index, segment));
        }
        consumed = next_bound;
    }

    if segments.len() <= 1 {
        return;
    }

    segments.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    let mut first_leading = Vec::new();
    if let Some((_, _, first_segment)) = segments.first_mut() {
        while first_segment
            .first()
            .is_some_and(|line| line.trim().is_empty() && !is_definition_continuation(line))
        {
            first_leading.push(first_segment.remove(0));
        }
    }

    let mut reordered = Vec::with_capacity(end - start);
    if prefix_len > 0 {
        reordered.extend(lines[start..start + prefix_len].iter().cloned());
    }

    for (idx, (_, _, segment)) in segments.into_iter().enumerate() {
        reordered.extend(segment);
        if idx == 0 && !first_leading.is_empty() {
            reordered.append(&mut first_leading);
        }
    }

    if reordered.len() == end - start {
        lines[start..end].clone_from_slice(&reordered);
    }
}
