//! Inline wrapping helpers that keep code spans intact.
//!
//! These functions operate on token streams so `wrap_text` can preserve
//! inline code, links, and trailing punctuation without reimplementing the
//! grouping logic in multiple places.

#[cfg(test)]
mod footnote_tests;
mod fragment;
mod postprocess;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
use std::ops::Range;

use fragment::{InlineFragment, width_as_f64};
use postprocess::{merge_whitespace_only_lines, rebalance_atomic_tails};
use textwrap::wrap_algorithms::wrap_first_fit;
use unicode_width::UnicodeWidthStr;

use super::tokenize;

/// Marks how a grouped token span should behave during wrapping.
#[derive(Copy, Clone, PartialEq, Eq)]
enum SpanKind {
    /// Treat the span as ordinary prose.
    General,
    /// Treat the span as an inline code sequence.
    Code,
    /// Treat the span as a Markdown link or image link.
    Link,
    /// Treat the span as a GitHub Flavoured Markdown footnote reference.
    FootnoteRef,
}
fn is_opening_punct(c: char) -> bool { matches!(c, '(' | '[') || "（［【《「『".contains(c) }
fn is_trailing_punct(c: char) -> bool {
    // ASCII closers + common Unicode closers and word-final punctuation
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '"' | '\''
    ) || "…—–»›）］】》」』、。，：；！？”.’".contains(c)
}

/// Returns whether `token` already looks like a complete Markdown link.
///
/// The `token` parameter is the rendered fragment text to inspect. The return
/// value is `true` only for complete inline links or image links, and this
/// helper never panics.
fn looks_like_link(token: &str) -> bool {
    (token.starts_with('[') || token.starts_with("!["))
        && token.contains("](")
        && token.ends_with(')')
}

/// Returns whether `token` looks like a complete GFM footnote reference.
///
/// The `token` parameter is the rendered fragment text to inspect. The return
/// value is `true` for the compact `[^label]` shape when `label` is non-empty.
/// This helper intentionally leaves label validation to the Markdown parser.
fn looks_like_footnote_ref(token: &str) -> bool {
    token
        .strip_prefix("[^")
        .and_then(|label| label.strip_suffix(']'))
        .is_some_and(|label| !label.is_empty())
}

/// Returns whether `token` ends with an inline footnote reference.
///
/// The wrapper can group `word.[^label]` into a single fragment to avoid
/// separating sentence punctuation from the marker. This predicate recognises
/// that suffix shape without treating arbitrary prose as a standalone marker.
fn ends_with_footnote_ref(token: &str) -> bool {
    let Some(start) = token.rfind("[^") else {
        return false;
    };

    looks_like_footnote_ref(&token[start..])
}

/// Returns whether `token` contains only Unicode whitespace.
///
/// The `token` parameter is the rendered fragment text to inspect. The return
/// value is `true` when every character is whitespace, and this helper never
/// panics.
fn is_whitespace_token(token: &str) -> bool { token.chars().all(char::is_whitespace) }

/// Returns whether `token` is a complete inline code span.
///
/// The `token` parameter is the rendered fragment text to inspect. The return
/// value is `true` only for complete backtick-delimited spans, and this helper
/// never panics.
fn is_inline_code_token(token: &str) -> bool { token.starts_with('`') && token.ends_with('`') }

/// Returns the substring beginning at the first Markdown link opener after any
/// leading opener punctuation.
///
/// Non-link openers such as `(` are skipped, but a leading `[` or `![` that
/// begins a link is preserved so opener-coupled links classify correctly.
fn link_text_after_leading_openers(text: &str) -> &str {
    let mut rest = text;
    while !rest.is_empty() {
        if rest.starts_with('[') || rest.starts_with("![") {
            return rest;
        }
        let Some(ch) = rest.chars().next() else {
            break;
        };
        if is_opening_punct(ch) {
            rest = &rest[ch.len_utf8()..];
        } else {
            break;
        }
    }
    rest
}

/// Strips one outer wrapper closing character from a link candidate when present.
fn strip_outer_link_wrapper_suffix(text: &str) -> Option<&str> {
    let last = text.chars().next_back()?;
    if matches!(last, ')' | ']' | '）' | '］' | '」' | '』' | '》') {
        Some(&text[..text.len() - last.len_utf8()])
    } else {
        None
    }
}

/// Returns whether rendered fragment text contains a Markdown link, including
/// links wrapped in outer opener punctuation.
fn fragment_is_link(text: &str) -> bool {
    if looks_like_link(text) {
        return true;
    }
    let mut candidate = link_text_after_leading_openers(text);
    while !candidate.is_empty() {
        if looks_like_link(candidate) {
            return true;
        }
        let Some(next) = strip_outer_link_wrapper_suffix(candidate) else {
            break;
        };
        candidate = next;
    }
    false
}

/// Extends a grouped span over trailing punctuation tokens and updates `width`.
///
/// `tokens` supplies the token stream, `j` is the next token index to inspect,
/// and `width` accumulates the display width of the current span. The return
/// value is the exclusive end index after any attached punctuation, and the
/// caller must pass a valid starting index within `tokens`.
fn extend_punctuation(tokens: &[String], mut j: usize, width: &mut usize) -> usize {
    while j < tokens.len() && tokens[j].chars().all(is_trailing_punct) {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
    }
    j
}

/// Decide whether whitespace between grouped tokens should stay attached to the
/// current span.
///
/// Links absorb following whitespace when another link, inline code span, or
/// punctuation immediately follows so that rendered Markdown keeps those items
/// together. Code spans are only coupled with trailing punctuation so that two
/// adjacent code spans can break across lines, but `code`, style suffixes still
/// cling to the preceding span.
fn should_couple_whitespace(kind: SpanKind, next_token: Option<&String>) -> bool {
    match (kind, next_token) {
        (SpanKind::Link, Some(next))
            if looks_like_link(next)
                || is_inline_code_token(next)
                || next.chars().all(is_trailing_punct) =>
        {
            true
        }
        (SpanKind::Code, Some(next)) if next.chars().all(is_trailing_punct) => true,
        _ => false,
    }
}

/// Merges a backtick-opened code span into one grouped span and updates
/// `width`.
///
/// `tokens` is the token stream, `i` is the index of a lone backtick opener,
/// and `width` accumulates the grouped display width. The return value is the
/// exclusive end index after the closing backtick and any attached
/// punctuation. This helper relies on the invariant that `tokens[i]` is a lone
/// backtick token.
#[inline]
fn merge_code_span(tokens: &[String], i: usize, width: &mut usize) -> usize {
    debug_assert!(
        tokens[i] == "`",
        "merge_code_span requires a single backtick opener"
    );
    let mut j = i + 1;
    while j < tokens.len() && tokens[j] != "`" {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
    }
    if j < tokens.len() {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
        j = extend_punctuation(tokens, j, width);
    }
    j
}

/// Extends `end` by one token and any trailing punctuation that follows it.
fn absorb_token_and_trailing_punctuation(
    tokens: &[String],
    end: usize,
    width: &mut usize,
) -> usize {
    *width += UnicodeWidthStr::width(tokens[end].as_str());
    extend_punctuation(tokens, end + 1, width)
}

/// Couples an adjacent footnote reference into the current span when appropriate.
///
/// General prose spans require sentence punctuation immediately before the
/// marker. Code and link spans already absorb trailing punctuation, so an
/// adjacent footnote reference is always coupled.
fn try_couple_footnote_reference(
    tokens: &[String],
    end: usize,
    kind: SpanKind,
    width: &mut usize,
) -> Option<(SpanKind, usize)> {
    let token = tokens.get(end)?;
    if !looks_like_footnote_ref(token) {
        return None;
    }

    match kind {
        SpanKind::General => {
            let previous = end
                .checked_sub(1)
                .and_then(|previous| tokens.get(previous))?;
            if !previous.chars().last().is_some_and(is_trailing_punct) {
                return None;
            }
            Some((
                SpanKind::FootnoteRef,
                absorb_token_and_trailing_punctuation(tokens, end, width),
            ))
        }
        SpanKind::Code | SpanKind::Link => Some((
            kind,
            absorb_token_and_trailing_punctuation(tokens, end, width),
        )),
        SpanKind::FootnoteRef => None,
    }
}

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
        if is_inline_code_token(next) {
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

    if tokens[start] == "`" {
        kind = SpanKind::Code;
        end = merge_code_span(tokens, start, &mut width);
    } else if is_inline_code_token(&tokens[start]) {
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

        if token.chars().all(is_trailing_punct) {
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
        let is_code = is_inline_code_token(token);
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
        buffer.push(fragment);
        let wrapped = wrap_first_fit(&buffer, &[width_as_f64(width)]);
        let raw_lines = wrapped.iter().map(|line| line.to_vec()).collect::<Vec<_>>();
        let mut grouped_lines = merge_whitespace_only_lines(&raw_lines);
        rebalance_atomic_tails(&mut grouped_lines, width);

        if grouped_lines.len() == 1 {
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
