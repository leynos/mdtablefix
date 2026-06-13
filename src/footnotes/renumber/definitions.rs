//! Definition scanning helpers for footnote renumbering.
//!
//! The parent module keeps the top-level rewrite flow, while this submodule
//! owns the detail-heavy definition parsing and scanning machinery. The
//! sibling [`reorder`](super::reorder) module owns block reordering, so each
//! source file stays readable and within the repository size limit.

use std::collections::HashMap;

use tracing::trace;

use super::{
    FOOTNOTE_LINE_RE,
    footnote_block_range,
    has_existing_footnote_block,
    is_definition_continuation,
    is_fence_line,
    parse_definition,
    rewrite_tokens,
};

/// Rewrite plan for a single footnote-definition line.
///
/// Each instance describes one definition that should replace the source
/// line at `index` once renumbering completes. Fields are populated once at
/// construction; the parent module treats the value as read-only state.
#[derive(Clone)]
pub(super) struct DefinitionLine {
    /// Zero-based row of the definition within the original `lines` slice.
    pub(super) index: usize,
    /// New sequential footnote number assigned to this definition.
    pub(super) new_number: usize,
    /// Fully rewritten line, including any leading indent and prefix, ready
    /// to be stored back into `lines[index]`.
    pub(super) line: String,
}

/// Buffered numeric-list line that may become a footnote definition.
///
/// Candidates are collected on the first pass and finalized in reverse so
/// they pick up sequential numbers after explicit `[^n]:` definitions have
/// been assigned. All fields are populated at construction and not mutated
/// afterwards.
pub(super) struct NumericCandidate {
    /// Zero-based row in the original `lines` slice.
    index: usize,
    /// Original ordered-list number to renumber.
    number: usize,
    /// Indentation preserved from the source line.
    indent: String,
    /// Whitespace separating the ordered-list marker from the body.
    whitespace: String,
    /// Definition body, still containing any inline `[^n]` references that
    /// will be rewritten using the final mapping.
    rest: String,
}

/// Aggregated output of a single definition scan.
///
/// Returned by [`collect_definition_updates`]. Treat both fields as a
/// matched pair: `is_definition_line` is indexed by source row and
/// `definitions` is keyed by `DefinitionLine::index`.
pub(super) struct DefinitionUpdates {
    /// Rewrite plans for every definition encountered. Explicit `[^n]:`
    /// definitions appear in scan order; promoted numeric candidates follow
    /// in reverse scan order (bottom-up), as required by
    /// `finalize_numeric_candidates`.
    pub(super) definitions: Vec<DefinitionLine>,
    /// `is_definition_line[i]` is `true` when row `i` of the source slice is
    /// the header of a footnote definition (existing or freshly promoted),
    /// so the caller can skip reference rewriting on that line.
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

/// Returns the exclusive end row of the definition segment that begins at
/// `start`, scanning no further than `block_end`.
///
/// A segment absorbs continuation lines and blank lines that merely separate
/// wrapped continuation text, but stops at the next `[^n]:` definition header
/// or any other non-continuation content. Shared with the sibling
/// [`reorder`](super::reorder) module so segment boundaries are computed
/// identically during scanning and reordering.
pub(super) fn definition_segment_end(lines: &[String], start: usize, block_end: usize) -> usize {
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
    parts: super::parsing::DefinitionParts<'_>,
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

/// Parses `line` at row `index` into a [`NumericCandidate`] if it matches
/// the footnote-line pattern. Returns `None` when the line is not an
/// ordered-list candidate or the captured number fails to parse.
pub(super) fn numeric_candidate_from_line(line: &str, index: usize) -> Option<NumericCandidate> {
    let caps = FOOTNOTE_LINE_RE.captures(line)?;
    let indent = caps.name("indent").map_or("", |m| m.as_str()).to_string();
    let num_match = caps.name("num")?;
    let rest_match = caps.name("rest")?;
    let number = num_match.as_str().parse::<usize>().ok()?;
    let rest = rest_match.as_str().to_string();
    let sep_start = num_match.end().saturating_add(1);
    let rest_start = rest_match.start();
    if sep_start > rest_start || sep_start > line.len() || rest_start > line.len() {
        return None;
    }
    let whitespace = line[sep_start..rest_start].to_string();
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
            trace!(
                index,
                numeric_list_range = ?state.numeric_list_range,
                skip_numeric_conversion = state.skip_numeric_conversion,
                "skipping numeric footnote candidate conversion"
            );
            continue;
        }
        if state.mapping.is_empty() && state.definitions.is_empty() {
            trace!(
                index,
                "skipping numeric footnote candidate before any reference mapping exists"
            );
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

/// Scans `lines` for footnote definitions and numeric candidates, updating
/// the shared reference `mapping` as numbers are assigned.
///
/// `mapping` is mutated in place to record every `(original, new)` pairing
/// discovered while building the [`DefinitionUpdates`] plan. Returns one
/// [`DefinitionLine`] per definition or promoted candidate, in scan order,
/// together with a per-row `is_definition_line` bitmap so the caller can
/// suppress reference rewriting on those rows.
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

/// Applies the rewrite plan in `definitions` to `lines`, replacing each
/// `lines[definition.index]` with `definition.line`. Other rows are left
/// untouched.
pub(super) fn rewrite_definition_headers(lines: &mut [String], definitions: &[DefinitionLine]) {
    for definition in definitions {
        lines[definition.index].clone_from(&definition.line);
    }
}

#[cfg(test)]
mod tests;
