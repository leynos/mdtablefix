//! Sequential renumbering of footnote references and definitions.

use std::{collections::HashMap, sync::LazyLock};

use regex::{Captures, Match, Regex};

/// Owns the footnote-definition parsing and scanning machinery, delegated
/// from the parent module so each source file remains readable and within
/// the repository size limit.
mod definitions;
/// Owns footnote-definition block reordering, kept separate from the
/// scanning machinery in [`definitions`] so each source file stays within
/// the repository size limit.
mod reorder;
mod parsing {
    //! Re-exports [`DefinitionParts`](super::super::parsing::DefinitionParts)
    //! from the parent module so [`definitions`](super::definitions) can use
    //! `super::parsing::DefinitionParts` without depending directly on its
    //! grandparent path. This is an internal alias; no new types belong here.

    pub(super) use super::super::parsing::DefinitionParts;
}

#[cfg(test)]
use definitions::numeric_candidate_from_line;
use definitions::{DefinitionUpdates, collect_definition_updates, rewrite_definition_headers};
use reorder::reorder_definition_block;
use tracing::debug;

use super::{
    lists::{footnote_block_range, has_existing_footnote_block, trimmed_range},
    parsing::{FOOTNOTE_LINE_RE, is_definition_continuation, parse_definition},
};
use crate::{
    textproc::{Token, push_original_token, tokenize_markdown},
    wrap::FenceTracker,
};

/// Finds numeric GFM references that may need sequential renumbering.
///
/// Definition headers use the same shape, so callers must run the match
/// through [`is_definition_like`] before treating it as prose.
static FOOTNOTE_REF_RE: LazyLock<Regex> = lazy_regex!(
    r"\[\^(?P<num>\d+)\]",
    "footnote reference pattern should compile",
);

/// Checks that text before a reference contains only definition prefixes.
///
/// Whitespace and blockquote markers are valid before a definition header;
/// other prose means the matching reference is ordinary document content.
fn matches_definition_prefix(prefix: &str) -> bool {
    let mut remaining = prefix;
    loop {
        remaining = remaining.trim_start_matches(char::is_whitespace);
        if remaining.is_empty() {
            return true;
        }
        if let Some(stripped) = remaining.strip_prefix('>') {
            remaining = stripped;
            continue;
        }
        return false;
    }
}

/// Determines whether a reference-shaped match is part of a definition header.
///
/// This guard prevents renumbering the identifier in `[^n]:` before the
/// definition rewrite has applied the shared mapping.
fn is_definition_like(text: &str, mat: &Match) -> bool {
    if !matches_definition_prefix(&text[..mat.start()]) {
        return false;
    }
    let suffix = &text[mat.end()..];
    let trimmed = suffix.trim_start_matches(char::is_whitespace);
    if !trimmed.starts_with(':') {
        return false;
    }
    if suffix.len() == trimmed.len() && trimmed.starts_with("::") {
        return false;
    }
    parse_definition(text.trim_end()).is_some()
}

/// Rewrites prose references in one token text segment using `mapping`.
///
/// Definition-shaped matches are retained so headers and their bodies can be
/// rewritten as a unit by the definition scanner.
fn rewrite_refs_in_segment(text: &str, mapping: &HashMap<usize, usize>) -> String {
    FOOTNOTE_REF_RE
        .replace_all(text, |caps: &Captures| {
            let Some(mat) = caps.get(0) else {
                return String::new();
            };
            if is_definition_like(text, &mat) {
                return caps[0].to_string();
            }
            caps["num"]
                .parse::<usize>()
                .ok()
                .and_then(|number| mapping.get(&number).copied())
                .map_or_else(
                    || caps[0].to_string(),
                    |new_number| format!("[^{new_number}]"),
                )
        })
        .into_owned()
}

/// Rewrites references in text tokens while preserving non-text Markdown.
///
/// Code and other protected tokens are copied byte-for-byte so a footnote-like
/// string inside them remains literal content.
fn rewrite_tokens(text: &str, mapping: &HashMap<usize, usize>) -> String {
    let mut rewritten = String::with_capacity(text.len());
    for token in tokenize_markdown(text) {
        match token {
            Token::Text(segment) => {
                rewritten.push_str(&rewrite_refs_in_segment(segment, mapping));
            }
            other => push_original_token(&other, &mut rewritten),
        }
    }
    rewritten
}

/// Collect first-seen reference numbers from prose outside fenced blocks.
///
/// For example, a reference after a matching outer fence closer is eligible,
/// while a reference between the opener and closer remains literal.
fn collect_reference_mapping(lines: &[String]) -> HashMap<usize, usize> {
    let mut mapping = HashMap::new();
    let mut next = 1;
    let mut fences = FenceTracker::default();
    for line in lines {
        let fence = fences.observe_source_line(line);
        if fence.is_fence_marker || fence.is_in_fence {
            continue;
        }
        for token in tokenize_markdown(line) {
            if let Token::Text(text) = token {
                collect_reference_mapping_from_text(text, &mut mapping, &mut next);
            }
        }
    }
    mapping
}

/// Adds first-seen references from a text segment to the shared mapping.
///
/// Definition headers are excluded because their numbers describe definitions,
/// not the document order in which references are encountered.
fn collect_reference_mapping_from_text(
    text: &str,
    mapping: &mut HashMap<usize, usize>,
    next: &mut usize,
) {
    for caps in FOOTNOTE_REF_RE.captures_iter(text) {
        let Some(mat) = caps.get(0) else {
            continue;
        };
        if is_definition_like(text, &mat) {
            continue;
        }
        let Ok(number) = caps["num"].parse::<usize>() else {
            continue;
        };
        if mapping.contains_key(&number) {
            continue;
        }
        mapping.insert(number, *next);
        *next += 1;
    }
}

/// Locates the trailing contiguous block containing footnote definitions.
///
/// Continuation and blank lines stay with the block, while unrelated trailing
/// prose prevents reordering a partial or non-footnote suffix.
fn footnote_definition_block_range(lines: &[String]) -> Option<(usize, usize)> {
    let (mut start, end) = trimmed_range(lines, |line| {
        line.trim().is_empty()
            || parse_definition(line).is_some()
            || is_definition_continuation(line)
    });
    while start < end
        && parse_definition(&lines[start]).is_none()
        && !lines[start].trim().is_empty()
    {
        start += 1;
    }
    if start < end
        && lines[start..end]
            .iter()
            .any(|line| parse_definition(line).is_some())
    {
        Some((start, end))
    } else {
        None
    }
}

/// Rewrite eligible prose references while preserving definitions and fences.
///
/// For example, a reference after a blockquote fence ends is rewritten, while
/// a reference within that fence remains unchanged.
fn apply_mapping_to_lines(
    lines: &mut [String],
    mapping: &HashMap<usize, usize>,
    is_definition_line: &[bool],
) {
    let mut fences = FenceTracker::default();
    for (idx, line) in lines.iter_mut().enumerate() {
        let fence = fences.observe_source_line(line);
        if fence.is_fence_marker
            || fence.is_in_fence
            || is_definition_line.get(idx).copied().unwrap_or(false)
        {
            continue;
        }
        *line = rewrite_tokens(line, mapping);
    }
}

/// Plans the renumbering of `lines`: the reference mapping and the definitions.
///
/// Returns [`None`] when there is nothing to renumber, which covers both a
/// document with neither references nor definitions and a document whose
/// references are left alone because an explicit `[^n]:` block already exists
/// elsewhere — such a block is maintained outside the formatter, so rewriting
/// the references that point into it would clobber it.
fn plan_renumbering(lines: &[String]) -> Option<(HashMap<usize, usize>, DefinitionUpdates)> {
    let mut mapping = collect_reference_mapping(lines);
    let definitions = collect_definition_updates(lines, &mut mapping);

    if mapping.is_empty() && definitions.definitions.is_empty() {
        return None;
    }

    if definitions.definitions.is_empty() && lines.iter().any(|line| FOOTNOTE_LINE_RE.is_match(line))
    {
        return None;
    }

    Some((mapping, definitions))
}

/// Rewrites footnote labels — references and definition headers — in place.
///
/// This is the length-changing half of [`renumber_footnotes`]: a reference such
/// as `[^10]` becomes `[^1]` once the distinct references are numbered by first
/// encounter, and a definition header is rewritten from the same mapping, so
/// both narrow the line they sit on and every pass that measures text has to run
/// after them. It is separate from the rest so the caller can place the two
/// halves either side of those passes; see `process_stream_inner`.
///
/// The mapping is applied in full, which is what leaves the document's numbers
/// final: a later [`renumber_footnotes`] call then recomputes an identity
/// mapping and has only the block structure left to settle. Rewriting the
/// references without the headers would not, because a definition header keeps
/// the number the mapping gave the reference that pointed at it, and a header
/// nobody referenced keeps its own — after the references have moved, the two
/// are indistinguishable to a second scan.
///
/// Lines inside fenced code blocks, and the definition rows themselves, are
/// never rewritten as references; a definition header is rewritten from its
/// parsed parts instead.
///
/// The scan promotes a trailing ordered-list item whose number some reference
/// shares into a definition header, because the two are matched by that number
/// — a bare `error.3` and the item `3.` are one footnote — and the match only
/// holds while the reference still carries the number it was written with.
pub(super) fn renumber_labels(lines: &mut [String]) {
    if let Some((mapping, definitions)) = plan_renumbering(lines) {
        apply_mapping_to_lines(lines, &mapping, &definitions.is_definition_line);
        rewrite_definition_headers(lines, &definitions.definitions);
        debug!(
            references = mapping.len(),
            definitions = definitions.definitions.len(),
            "renumbering footnote references and definition headers"
        );
    }
}
/// Sequentially renumbers GFM footnote references and definitions in `lines`.
///
/// The input is mutated in place. Each distinct `[^n]` reference encountered
/// in document order is mapped to the next available positive integer,
/// starting at `1`, with the very first reference always becoming `[^1]`.
/// Repeat references share the previously assigned number. Definitions are
/// rewritten using the same mapping; if a numeric ordered-list item is being
/// promoted into a definition it is also assigned the next available number.
///
/// If no references and no definitions are found the input is left untouched.
/// If references exist but no definitions do, and the document already
/// contains an explicit `[^n]:` block elsewhere, references are also left
/// untouched to avoid clobbering an externally maintained block.
///
/// When the document carries a footnote-definition block its contents are
/// reordered so definitions appear sorted by their new sequential numbers,
/// with continuation lines kept attached to their definition. Lines inside
/// fenced code blocks are never rewritten.
///
/// The reference half also runs on its own, as [`renumber_references`], before
/// the passes that measure text; by the time this whole-document form reaches
/// it the references are already sequential and the second rewriting leaves
/// them as they are.
pub(super) fn renumber_footnotes(lines: &mut [String]) {
    let Some((mapping, definitions)) = plan_renumbering(lines) else {
        return;
    };

    apply_mapping_to_lines(lines, &mapping, &definitions.is_definition_line);

    rewrite_definition_headers(lines, &definitions.definitions);

    if let Some((start, end)) = footnote_definition_block_range(lines) {
        reorder_definition_block(lines, start, end, &definitions.definitions);
    }
}

#[cfg(test)]
mod tests;
