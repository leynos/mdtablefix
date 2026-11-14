//! Line fitting for inline wrapping.
//!
//! This module turns the grouped token spans produced by `span_grouping` into
//! rendered `InlineFragment`s, fits them with `textwrap`, and renders the
//! result back into Markdown lines that keep code spans, links, and footnote
//! references atomic.

use std::ops::Range;

use textwrap::wrap_algorithms::wrap_first_fit;
use unicode_width::UnicodeWidthStr;

use super::{
    fragment::{InlineFragment, width_as_f64},
    normalize::normalize_footnote_ref_spacing,
    postprocess::{merge_whitespace_only_lines, rebalance_atomic_tails},
    predicates::is_whitespace_token,
    span_grouping::determine_token_span_observed,
    touching::join_touching_fragments,
};
use crate::wrap::{
    observer::{FragmentKind, ObserverHandle},
    tokenize,
};

/// Returns whether `token` is sentence punctuation to glue onto a code span.
fn glues_to_code_span(token: &str) -> bool { token.len() == 1 && ".?!,:;".contains(token) }

/// Appends the token span into the rendered fragment buffer `text`.
///
/// `tokens` supplies the source tokens and `span` identifies the grouped range
/// to copy. This helper mutates `text` in place and preserves the invariant
/// that punctuation after code spans keeps its original Markdown spacing.
fn push_span_text(text: &mut String, tokens: &[String], span: Range<usize>) {
    for token in &tokens[span] {
        if glues_to_code_span(token) && text.trim_end().ends_with('`') {
            text.truncate(text.trim_end_matches(char::is_whitespace).len());
        }
        text.push_str(token);
    }
}

/// Builds Markdown-aware fragments from the segmented token stream `tokens`.
///
/// The return value preserves token order while grouping inline code, links,
/// and whitespace runs into `InlineFragment` values with precomputed widths.
/// This helper never panics when `tokens` is well-formed.
fn build_fragments(tokens: &[String], observer: &mut ObserverHandle<'_>) -> Vec<InlineFragment> {
    let mut fragments: Vec<InlineFragment> = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let (group_end, _group_width) = determine_token_span_observed(tokens, i, observer);
        let span = i..group_end;
        let text = if tokens[i..group_end]
            .iter()
            .all(|token| is_whitespace_token(token))
        {
            tokens[span].join("")
        } else {
            let mut text = String::new();
            push_span_text(&mut text, tokens, span);
            text
        };
        fragments.push(InlineFragment::new_observed(text, observer));
        i = group_end;
    }

    join_touching_fragments(fragments, observer)
}

/// Returns whether `line` contains one link fragment.
fn is_single_link_line(line: &[InlineFragment]) -> bool {
    line.len() == 1 && line[0].kind == FragmentKind::Link
}

/// Returns the total display width of a fragment line.
fn fragment_line_width(line: &[InlineFragment]) -> usize {
    line.iter().map(|fragment| fragment.width).sum()
}

/// Returns whether `line_width` uses all of `width` but for one column.
fn fills_width(line_width: usize, width: usize) -> bool {
    line_width == width || line_width + 1 == width
}

/// Returns whether `fragment` reads as ordinary prose after a boundary link.
fn is_prose(fragment: &InlineFragment) -> bool { fragment.is_whitespace() || fragment.is_plain() }

/// Returns whether `line` begins with a link followed by prose.
fn starts_with_boundary_link(line: &[InlineFragment]) -> bool {
    line.first()
        .is_some_and(|fragment| fragment.kind == FragmentKind::Link)
        && line.get(1).is_some_and(is_prose)
}

/// Splits a link first fragment from trailing prose after a boundary wrap.
fn split_boundary_link_line(
    previous_line: &[InlineFragment],
    line: &[InlineFragment],
    width: usize,
) -> Option<(Vec<InlineFragment>, Vec<InlineFragment>)> {
    if !fills_width(fragment_line_width(previous_line), width) || !starts_with_boundary_link(line) {
        return None;
    }

    Some((vec![line[0].clone()], line[1..].to_vec()))
}

/// Returns whether a boundary link fragment should be finalized now.
fn should_flush_boundary_link(
    lines: &[String],
    buffer: &[InlineFragment],
    next: &InlineFragment,
    width: usize,
) -> bool {
    lines.last().is_some_and(|line| {
        let rendered_width = UnicodeWidthStr::width(line.as_str());
        fills_width(rendered_width, width)
    }) && is_single_link_line(buffer)
        && is_prose(next)
}

/// Renders one wrapped fragment line back into Markdown text.
///
/// `line` supplies the fragments to render. `is_final_output_line` determines
/// whether a single trailing space may be trimmed. When
/// `strip_leading_carry_whitespace` is set, carry whitespace from the wrap
/// pipeline is removed from continuation lines only. The return value is the
/// emitted text for that line, and this helper preserves the invariant that
/// hard-break double spaces survive on the final output line.
fn render_line(
    line: &[InlineFragment],
    is_final_output_line: bool,
    strip_leading_carry_whitespace: bool,
) -> String {
    let mut text = line
        .iter()
        .map(|fragment| fragment.text.as_str())
        .collect::<String>();

    // A break the wrapper made must render as a soft break, so every trailing
    // space goes: two or more left at a line end would read as a hard break.
    // Only the final line keeps them, since there they are the source's own.
    if !is_final_output_line {
        text.truncate(text.trim_end_matches(' ').len());
    }

    if strip_leading_carry_whitespace {
        text = text.trim_start().to_string();
    }

    text
}

/// Groups the buffered fragments into lines no wider than `width`.
fn wrap_buffer(buffer: &[InlineFragment], width: usize) -> Vec<Vec<InlineFragment>> {
    let wrapped = wrap_first_fit(buffer, &[width_as_f64(width)]);
    let raw_lines = wrapped.iter().map(|line| line.to_vec()).collect::<Vec<_>>();
    let mut grouped_lines = merge_whitespace_only_lines(&raw_lines, width);
    rebalance_atomic_tails(&mut grouped_lines, width);
    grouped_lines
}

/// Splits a boundary link line from its trailing prose and flushes both.
///
/// Returns whether the boundary split applied. A layout with fewer than two
/// lines has no penultimate line to compare against, so the split is declined
/// rather than indexed past the start of the slice.
fn flush_boundary_link(
    grouped_lines: &[Vec<InlineFragment>],
    width: usize,
    lines: &mut Vec<String>,
    buffer: &mut Vec<InlineFragment>,
) -> bool {
    let split = grouped_lines
        .len()
        .checked_sub(2)
        .and_then(|penultimate| grouped_lines.get(penultimate))
        .zip(grouped_lines.last())
        .and_then(|(previous, line)| split_boundary_link_line(previous, line, width));
    let Some((link_line, remaining_line)) = split else {
        return false;
    };

    for line in &grouped_lines[..grouped_lines.len() - 1] {
        lines.push(render_line(line, false, !lines.is_empty()));
    }
    lines.push(render_line(&link_line, false, !lines.is_empty()));
    *buffer = remaining_line;
    true
}

/// Renders every grouped line except the last into `lines`.
fn flush_all_but_last(grouped_lines: &mut [Vec<InlineFragment>], lines: &mut Vec<String>) {
    for line in &grouped_lines[..grouped_lines.len() - 1] {
        lines.push(render_line(line, false, !lines.is_empty()));
    }
}

/// Fits one more fragment into `buffer`, flushing completed lines into `lines`.
fn fit_fragment(
    fragment: InlineFragment,
    width: usize,
    lines: &mut Vec<String>,
    buffer: &mut Vec<InlineFragment>,
) {
    if should_flush_boundary_link(lines, buffer, &fragment, width) {
        lines.push(render_line(buffer, false, !lines.is_empty()));
        buffer.clear();
        if fragment.is_whitespace() {
            return;
        }
    }

    buffer.push(fragment);
    let mut grouped_lines = wrap_buffer(buffer, width);
    // A wrap of whitespace-only content can fold away entirely, leaving no
    // line to split around. Treat that, like a single line, as "nothing to
    // flush yet" rather than indexing past the start of the layout.
    if grouped_lines.len() <= 1 {
        return;
    }
    if flush_boundary_link(&grouped_lines, width, lines, buffer) {
        return;
    }

    flush_all_but_last(&mut grouped_lines, lines);
    *buffer = grouped_lines.pop().unwrap_or_default();
}

/// Wraps inline Markdown `text` while reporting classification events to
/// `observer`.
///
/// This is the observer-threaded form of [`wrap_preserving_code`]; wrapping
/// behaviour and the meaning of `width` are identical, and it never panics for
/// valid input. Events are emitted only while `observer` is `Some`; passing
/// `None` runs the pure domain path and emits nothing.
///
/// Events carry borrowed data only. Deriving anything costlier than a copy —
/// Unicode length counts, for example — is the observer's responsibility, so an
/// adapter that gates on a disabled log level performs no per-event work.
/// Observers must record content-free metadata only and never the token text
/// itself.
pub(in crate::wrap) fn wrap_preserving_code_observed(
    text: &str,
    width: usize,
    observer: &mut ObserverHandle<'_>,
) -> Vec<String> {
    let tokens = tokenize::segment_inline_observed(text, observer);
    if tokens.is_empty() {
        return Vec::new();
    }

    let tokens = normalize_footnote_ref_spacing(&tokens);
    let fragments = build_fragments(&tokens, observer);
    let mut lines = Vec::new();
    let mut buffer: Vec<InlineFragment> = Vec::new();

    for fragment in fragments {
        fit_fragment(fragment, width, &mut lines, &mut buffer);
    }

    if !buffer.is_empty() {
        lines.push(render_line(&buffer, true, !lines.is_empty()));
    }

    lines
}
