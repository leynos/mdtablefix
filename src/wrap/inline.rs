//! Inline wrapping helpers that keep code spans intact.
//!
//! These functions operate on token streams so `wrap_text` can preserve
//! inline code, links, and trailing punctuation without reimplementing the
//! grouping logic in multiple places.

#[cfg(test)]
mod footnote_tests;
mod fragment;
mod postprocess;
mod predicates;
mod span_helpers;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

/// Returns whether `token` begins with a matched inline code fence, optionally
/// followed by a non-whitespace suffix such as an inflectional affix.
fn has_inline_code_structure(token: &str) -> bool { fragment::has_inline_code_structure(token) }

fn is_code_token(token: &str) -> bool {
    is_inline_code_token(token) || has_inline_code_structure(token)
}

use std::ops::Range;

use fragment::{InlineFragment, width_as_f64};
use postprocess::{merge_whitespace_only_lines, rebalance_atomic_tails};
use predicates::looks_like_link;
pub(in crate::wrap::inline) use predicates::{
    ends_with_footnote_ref,
    ends_with_hyphen_prefix,
    fragment_is_link,
    is_inline_code_token,
    is_opening_punct,
    is_trailing_punct,
    is_trailing_punctuation_token,
    is_whitespace_token,
    looks_like_footnote_ref,
};
use span_helpers::{
    SpanKind,
    absorb_token_and_trailing_punctuation,
    extend_punctuation,
    merge_code_span,
    should_couple_whitespace,
    try_couple_footnote_reference,
};
use textwrap::wrap_algorithms::wrap_first_fit;
use unicode_width::UnicodeWidthStr;

use super::tokenize;

/// Finds the next logical token group starting at `start`.
///
/// `tokens` is the segmented inline token stream and `start` is the first
/// token in the next candidate group. The return value is `(end, width)`,
/// where `end` is the exclusive end index of the grouped inline code span,
/// link, or plain fragment, and `width` is its Unicode display width. This
/// helper assumes `start < tokens.len()` and will panic if called out of
/// bounds.
pub(super) fn determine_token_span(tokens: &[String], start: usize) -> (usize, usize) {
    let mut end = start + 1;
    let mut width = UnicodeWidthStr::width(tokens[start].as_str());
    let mut kind = SpanKind::General;

    // Forward-couple opening punctuation to the next atomic span so wrapping
    // never leaves a lone `(` at the end of a line before inline code or a link.
    if tokens[start].chars().all(is_opening_punct)
        && let Some(next) = tokens.get(start + 1)
    {
        if is_code_token(next) {
            kind = SpanKind::Code;
            end += 1;
            width += UnicodeWidthStr::width(next.as_str());
            end = extend_punctuation(tokens, end, &mut width);
        } else if looks_like_link(next) {
            kind = SpanKind::Link;
            end += 1;
            width += UnicodeWidthStr::width(next.as_str());
            end = extend_punctuation(tokens, end, &mut width);
        }
    }

    // Forward-couple a hyphen-prefix token to the next inline code span so
    // wrapping never splits compounds such as `pre-`code`` at the hyphen.
    if kind == SpanKind::General
        && ends_with_hyphen_prefix(&tokens[start])
        && let Some(next) = tokens.get(end)
        && is_code_token(next)
    {
        kind = SpanKind::Code;
        width += UnicodeWidthStr::width(next.as_str());
        end += 1;
        end = extend_punctuation(tokens, end, &mut width);
    }

    if tokens[start] == "`" {
        kind = SpanKind::Code;
        end = merge_code_span(tokens, start, &mut width);
    } else if is_code_token(&tokens[start]) {
        kind = SpanKind::Code;
        end = extend_punctuation(tokens, end, &mut width);
    } else if looks_like_link(&tokens[start]) {
        kind = SpanKind::Link;
        end = extend_punctuation(tokens, end, &mut width);
    } else if looks_like_footnote_ref(&tokens[start]) {
        kind = SpanKind::FootnoteRef;
        end = extend_punctuation(tokens, end, &mut width);
    }

    while end < tokens.len() {
        let token = &tokens[end];
        if is_whitespace_token(token) {
            if should_couple_whitespace(kind, tokens.get(end + 1)) {
                width += UnicodeWidthStr::width(token.as_str());
                end += 1;
                continue;
            }

            break;
        }

        if is_trailing_punctuation_token(token) {
            if matches!(
                kind,
                SpanKind::Code | SpanKind::Link | SpanKind::FootnoteRef
            ) {
                width += UnicodeWidthStr::width(token.as_str());
                end += 1;
                continue;
            }
            break;
        }

        let is_link = looks_like_link(token);
        let is_code = is_code_token(token);
        // Footnote markers must be coupled before consecutive link/code chaining;
        // otherwise `[^N]` stays a separate wrap token even when punctuation is
        // already attached to the preceding atomic span.
        if let Some((next_kind, next_end)) =
            try_couple_footnote_reference(tokens, end, kind, &mut width)
        {
            kind = next_kind;
            end = next_end;
            continue;
        }

        if kind == SpanKind::Link && is_link {
            end = absorb_token_and_trailing_punctuation(tokens, end, &mut width);
            continue;
        }

        if kind == SpanKind::Code && is_code {
            end = absorb_token_and_trailing_punctuation(tokens, end, &mut width);
            continue;
        }

        break;
    }

    (end, width)
}

/// Re-exports the test-only helper that joins punctuation onto a prior code
/// line when `current` is empty.
#[cfg(test)]
pub(super) use test_support::attach_punctuation_to_previous_line;

/// Appends the token span into the rendered fragment buffer `text`.
///
/// `tokens` supplies the source tokens and `span` identifies the grouped range
/// to copy. This helper mutates `text` in place and preserves the invariant
/// that punctuation after code spans keeps its original Markdown spacing.
fn push_span_text(text: &mut String, tokens: &[String], span: Range<usize>) {
    for token in &tokens[span] {
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
        let (group_end, _) = determine_token_span(tokens, i);
        let span = i..group_end;
        let text = if tokens[span.clone()]
            .iter()
            .all(|token| is_whitespace_token(token))
        {
            tokens[span].join("")
        } else {
            let mut text = String::new();
            push_span_text(&mut text, tokens, span);
            text
        };
        fragments.push(InlineFragment::new(text));
        i = group_end;
    }

    fragments
}

/// Returns whether `line` contains one atomic fragment.
fn is_single_atomic_line(line: &[InlineFragment]) -> bool { line.len() == 1 && line[0].is_atomic() }

/// Returns the total display width of a fragment line.
fn fragment_line_width(line: &[InlineFragment]) -> usize {
    line.iter().map(|fragment| fragment.width).sum()
}

/// Splits an atomic first fragment from trailing prose after a boundary wrap.
fn split_boundary_atomic_line(
    previous_line: &[InlineFragment],
    line: &[InlineFragment],
    width: usize,
) -> Option<(Vec<InlineFragment>, Vec<InlineFragment>)> {
    let previous_width = fragment_line_width(previous_line);
    if !(previous_width == width || previous_width + 1 == width)
        || !line.first().is_some_and(InlineFragment::is_atomic)
        || !line
            .get(1)
            .is_some_and(|fragment| fragment.is_whitespace() || fragment.is_plain())
    {
        return None;
    }

    Some((vec![line[0].clone()], line[1..].to_vec()))
}

/// Returns whether a boundary atomic fragment should be finalised now.
fn should_flush_boundary_atomic(
    lines: &[String],
    buffer: &[InlineFragment],
    next: &InlineFragment,
    width: usize,
) -> bool {
    lines.last().is_some_and(|line| {
        let rendered_width = UnicodeWidthStr::width(line.as_str());
        rendered_width == width || rendered_width + 1 == width
    }) && is_single_atomic_line(buffer)
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
/// `textwrap::wrap_algorithms::wrap_first_fit`, normalised with
/// `merge_whitespace_only_lines` plus `rebalance_atomic_tails`, and then
/// rendered back into `Vec<String>` output lines. `width` is measured in
/// Unicode display columns and must be at least one effective column after any
/// caller prefix handling. This helper never panics for valid input.
pub(super) fn wrap_preserving_code(text: &str, width: usize) -> Vec<String> {
    let tokens = tokenize::segment_inline(text);
    if tokens.is_empty() {
        return Vec::new();
    }

    let fragments = build_fragments(&tokens);
    let mut lines = Vec::new();
    let mut buffer: Vec<InlineFragment> = Vec::new();

    for fragment in fragments {
        if should_flush_boundary_atomic(&lines, &buffer, &fragment, width) {
            lines.push(render_line(&buffer, false, !lines.is_empty()));
            buffer.clear();
            if fragment.is_whitespace() {
                continue;
            }
        }

        buffer.push(fragment);
        let wrapped = wrap_first_fit(&buffer, &[width_as_f64(width)]);
        let raw_lines = wrapped.iter().map(|line| line.to_vec()).collect::<Vec<_>>();
        let mut grouped_lines = merge_whitespace_only_lines(&raw_lines);
        rebalance_atomic_tails(&mut grouped_lines, width);

        if grouped_lines.len() == 1 {
            continue;
        }

        if let Some((atomic_line, remaining_line)) = grouped_lines
            .get(grouped_lines.len() - 2)
            .zip(grouped_lines.last())
            .and_then(|(previous, line)| split_boundary_atomic_line(previous, line, width))
        {
            for line in &grouped_lines[..grouped_lines.len() - 1] {
                lines.push(render_line(line, false, !lines.is_empty()));
            }
            lines.push(render_line(&atomic_line, false, !lines.is_empty()));
            buffer = remaining_line;
            continue;
        }

        for line in &grouped_lines[..grouped_lines.len() - 1] {
            lines.push(render_line(line, false, !lines.is_empty()));
        }
        buffer = grouped_lines.pop().unwrap_or_default();
    }

    if !buffer.is_empty() {
        lines.push(render_line(&buffer, true, !lines.is_empty()));
    }

    lines
}
