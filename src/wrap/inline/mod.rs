//! Inline wrapping helpers that keep code spans intact.
//!
//! These functions operate on token streams so `wrap_text` can preserve
//! inline code, links, and trailing punctuation without reimplementing the
//! grouping logic in multiple places.

#[cfg(test)]
mod footnote_tests;
mod fragment;
mod month_names;
mod normalize;
mod postprocess;
mod predicates;
mod span_classification;
mod span_helpers;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod tracing_events;

/// Returns whether `token` begins with a matched inline code fence, optionally
/// followed by a non-whitespace suffix such as an inflectional affix.
fn has_inline_code_structure(token: &str) -> bool { fragment::has_inline_code_structure(token) }

/// Return whether a token is an atomic code fragment for wrapping purposes.
///
/// A complete inline-code token and a code span followed by an attached
/// inflectional suffix must both stay together when calculating line breaks.
fn is_code_token(token: &str) -> bool {
    is_inline_code_token(token) || has_inline_code_structure(token)
}

use std::ops::Range;

use fragment::{InlineFragment, width_as_f64};
use normalize::normalize_footnote_ref_spacing;
use postprocess::{merge_whitespace_only_lines, rebalance_atomic_tails};
pub(in crate::wrap::inline) use predicates::{
    ends_with_footnote_ref,
    fragment_is_link,
    is_inline_code_token,
    is_opening_punct,
    is_trailing_punct,
    is_whitespace_token,
    looks_like_bracketed_reference,
    looks_like_footnote_ref,
};
pub(super) use span_classification::determine_token_span;
use span_helpers::SpanKind;
/// Re-exports the test-only helper that joins punctuation onto a prior code
/// line when `current` is empty.
#[cfg(test)]
pub(super) use test_support::attach_punctuation_to_previous_line;
use textwrap::wrap_algorithms::wrap_first_fit;
use unicode_width::UnicodeWidthStr;

use super::tokenize;

/// Appends the token span into the rendered fragment buffer `text`.
///
/// `tokens` supplies the source tokens and `span` identifies the grouped range
/// to copy. This helper mutates `text` in place and preserves the invariant
/// that punctuation after code spans keeps its original Markdown spacing.
fn push_span_text(text: &mut String, tokens: &[String], span: Range<usize>) {
    let Some(span_tokens) = tokens.get(span) else {
        return;
    };
    for token in span_tokens {
        if token.len() == 1 && ".?!,:;".contains(token) && text.trim_end().ends_with('`') {
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
fn build_fragments(tokens: &[String]) -> Vec<InlineFragment> {
    let mut fragments: Vec<InlineFragment> = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let (group_end, _group_width) = determine_token_span(tokens, i);
        let Some(span_tokens) = tokens.get(i..group_end).filter(|span| !span.is_empty()) else {
            break;
        };
        let text = if span_tokens.iter().all(|token| is_whitespace_token(token)) {
            span_tokens.join("")
        } else {
            let mut text = String::new();
            push_span_text(&mut text, tokens, i..group_end);
            text
        };
        fragments.push(InlineFragment::new(text));
        i = group_end;
    }

    fragments
}

/// Returns whether `line` contains one link fragment.
fn is_single_link_line(line: &[InlineFragment]) -> bool {
    line.len() == 1
        && line
            .first()
            .is_some_and(|fragment| fragment.kind == fragment::FragmentKind::Link)
}

/// Returns the total display width of a fragment line.
fn fragment_line_width(line: &[InlineFragment]) -> usize {
    line.iter().map(|fragment| fragment.width).sum()
}

/// Splits a link first fragment from trailing prose after a boundary wrap.
fn split_boundary_link_line(
    previous_line: &[InlineFragment],
    line: &[InlineFragment],
    width: usize,
) -> Option<(Vec<InlineFragment>, Vec<InlineFragment>)> {
    let previous_width = fragment_line_width(previous_line);
    if !(previous_width == width || previous_width + 1 == width)
        || !line
            .first()
            .is_some_and(|fragment| fragment.kind == fragment::FragmentKind::Link)
        || !line
            .get(1)
            .is_some_and(|fragment| fragment.is_whitespace() || fragment.is_plain())
    {
        return None;
    }

    Some((vec![line.first()?.clone()], line.get(1..)?.to_vec()))
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
        rendered_width == width || rendered_width + 1 == width
    }) && is_single_link_line(buffer)
        && (next.is_whitespace() || next.is_plain())
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

    if !is_final_output_line && text.ends_with(' ') && !text.ends_with("  ") {
        text.pop();
    }

    if strip_leading_carry_whitespace {
        text = text.trim_start().to_string();
    }

    text
}

/// Wraps inline Markdown `text` without splitting code spans or links.
///
/// `text` is tokenised into `InlineFragment`s, fitted with
/// `textwrap::wrap_algorithms::wrap_first_fit`, normalized with
/// `merge_whitespace_only_lines` plus `rebalance_atomic_tails`, and then
/// rendered back into `Vec<String>` output lines. `width` is measured in
/// Unicode display columns and must be at least one effective column after any
/// caller prefix handling. This helper never panics for valid input.
pub(super) fn wrap_preserving_code(text: &str, width: usize) -> Vec<String> {
    let tokens = tokenize::segment_inline(text);
    if tokens.is_empty() {
        return Vec::new();
    }

    let tokens = normalize_footnote_ref_spacing(&tokens);
    let fragments = build_fragments(&tokens);
    let mut lines = Vec::new();
    let mut buffer: Vec<InlineFragment> = Vec::new();

    for fragment in fragments {
        if should_flush_boundary_link(&lines, &buffer, &fragment, width) {
            lines.push(render_line(&buffer, false, !lines.is_empty()));
            buffer.clear();
            if fragment.is_whitespace() {
                continue;
            }
        }

        buffer.push(fragment);
        let wrapped = wrap_first_fit(&buffer, &[width_as_f64(width)]);
        let raw_lines = wrapped.iter().map(|line| line.to_vec()).collect::<Vec<_>>();
        let mut grouped_lines = merge_whitespace_only_lines(&raw_lines, width);
        rebalance_atomic_tails(&mut grouped_lines, width);

        emit_completed_lines(&mut lines, &mut buffer, grouped_lines, width);
    }

    if !buffer.is_empty() {
        lines.push(render_line(&buffer, true, !lines.is_empty()));
    }

    lines
}

/// Emit finished wrap lines and retain the final fragments for the next pass.
fn emit_completed_lines(
    lines: &mut Vec<String>,
    buffer: &mut Vec<InlineFragment>,
    mut grouped_lines: Vec<Vec<InlineFragment>>,
    width: usize,
) {
    if grouped_lines.len() < 2 {
        return;
    }

    let split_link = grouped_lines
        .get(grouped_lines.len() - 2)
        .zip(grouped_lines.last())
        .and_then(|(previous, line)| split_boundary_link_line(previous, line, width));
    for line in grouped_lines.iter().take(grouped_lines.len() - 1) {
        lines.push(render_line(line, false, !lines.is_empty()));
    }
    if let Some((link_line, remaining_line)) = split_link {
        lines.push(render_line(&link_line, false, !lines.is_empty()));
        *buffer = remaining_line;
    } else {
        *buffer = grouped_lines.pop().unwrap_or_default();
    }
}

#[cfg(test)]
mod date_strategies;
