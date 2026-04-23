//! Inline wrapping helpers that keep code spans intact.
//!
//! These functions operate on token streams so `wrap_text` can preserve
//! inline code, links, and trailing punctuation without reimplementing the
//! grouping logic in multiple places.

mod postprocess;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
use std::ops::Range;

use postprocess::{merge_whitespace_only_lines, rebalance_atomic_tails};
use textwrap::{core::Fragment, wrap_algorithms::wrap_first_fit};
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
}

/// Returns whether a character should stay attached as trailing punctuation.
///
/// The `c` parameter is the candidate trailing character. The return value is
/// `true` only for punctuation that should remain coupled with a preceding
/// atomic span. This helper has no panics and assumes `c` is a single scalar
/// value from an already-tokenised fragment.
#[inline]
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

    if tokens[start] == "`" {
        kind = SpanKind::Code;
        end = merge_code_span(tokens, start, &mut width);
    } else if is_inline_code_token(&tokens[start]) {
        kind = SpanKind::Code;
        end = extend_punctuation(tokens, end, &mut width);
    } else if looks_like_link(&tokens[start]) {
        kind = SpanKind::Link;
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
            if matches!(kind, SpanKind::Code | SpanKind::Link) {
                width += UnicodeWidthStr::width(token.as_str());
                end += 1;
                continue;
            }
            break;
        }

        let is_link = looks_like_link(token);
        let is_code = is_inline_code_token(token);

        if kind == SpanKind::Link && is_link {
            width += UnicodeWidthStr::width(token.as_str());
            end += 1;
            end = extend_punctuation(tokens, end, &mut width);
            continue;
        }

        if kind == SpanKind::Code && is_code {
            width += UnicodeWidthStr::width(token.as_str());
            end += 1;
            end = extend_punctuation(tokens, end, &mut width);
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

/// Classifies an inline fragment for post-wrap heuristics.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FragmentKind {
    /// Marks a fragment that contains only whitespace.
    Whitespace,
    /// Marks a fragment that contains inline code.
    InlineCode,
    /// Marks a fragment that contains a Markdown link.
    Link,
    /// Marks a fragment that contains ordinary prose.
    Plain,
}

/// Stores rendered fragment text, width, and classification for wrapping.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InlineFragment {
    /// Holds the rendered fragment text that will be emitted unchanged.
    text: String,
    /// Stores the precomputed Unicode display width for `text`.
    width: usize,
    /// Records the fragment classification used by post-processing predicates.
    kind: FragmentKind,
}

impl InlineFragment {
    /// Builds a fragment from rendered `text`.
    ///
    /// The parameter is stored verbatim, while the return value carries the
    /// same text together with its Unicode display width and `FragmentKind`.
    /// This constructor never panics and preserves the invariant that `width`
    /// and `kind` are derived from `text` exactly once.
    fn new(text: String) -> Self {
        let width = UnicodeWidthStr::width(text.as_str());
        let kind = classify_fragment(text.as_str());
        Self { text, width, kind }
    }
    /// Returns whether this fragment contains only whitespace.
    ///
    /// The return value is `true` only for `FragmentKind::Whitespace`. This
    /// query never panics and does not inspect `text` again.
    fn is_whitespace(&self) -> bool { self.kind == FragmentKind::Whitespace }
    /// Returns whether this fragment must move as an atomic unit.
    ///
    /// The return value is `true` for inline code spans and links. This query
    /// never panics and relies on the invariant that `kind` was set at
    /// construction time.
    fn is_atomic(&self) -> bool {
        matches!(self.kind, FragmentKind::InlineCode | FragmentKind::Link)
    }
    /// Returns whether this fragment is ordinary prose.
    ///
    /// The return value is `true` only for `FragmentKind::Plain`. This query
    /// never panics and does not inspect `text` again.
    fn is_plain(&self) -> bool { self.kind == FragmentKind::Plain }
}

impl Fragment for InlineFragment {
    fn width(&self) -> f64 { width_as_f64(self.width) }
    fn whitespace_width(&self) -> f64 { 0.0 }
    fn penalty_width(&self) -> f64 { 0.0 }
}

/// Converts a display width into the `f64` representation required by
/// `textwrap`.
///
/// `width` is the precomputed display width of one fragment or line. The
/// return value is a saturating `f64` conversion for `wrap_first_fit`; values
/// above `u32::MAX` clamp to that limit. This helper never panics.
fn width_as_f64(width: usize) -> f64 { f64::from(u32::try_from(width).unwrap_or(u32::MAX)) }

/// Classifies rendered fragment `text` for later post-processing.
///
/// The parameter is the rendered fragment string, and the return value is a
/// `FragmentKind` used by cheap predicate helpers. This helper never panics
/// and keeps the invariant that classification is centralised in one place.
fn classify_fragment(text: &str) -> FragmentKind {
    if is_whitespace_token(text) {
        return FragmentKind::Whitespace;
    }
    let trimmed = text.trim_end_matches(is_trailing_punct);
    if is_inline_code_token(text) || is_inline_code_token(trimmed) {
        FragmentKind::InlineCode
    } else if looks_like_link(text) || looks_like_link(trimmed) {
        FragmentKind::Link
    } else {
        FragmentKind::Plain
    }
}

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
/// `line` supplies the fragments to render and `is_final_output_line`
/// determines whether a single trailing space may be trimmed. The return value
/// is the emitted text for that line, and this helper preserves the invariant
/// that hard-break double spaces survive on the final output line.
fn render_line(line: &[InlineFragment], is_final_output_line: bool) -> String {
    let mut text = line
        .iter()
        .map(|fragment| fragment.text.as_str())
        .collect::<String>();

    if !is_final_output_line && text.ends_with(' ') && !text.ends_with("  ") {
        text.pop();
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
            lines.push(render_line(line, false));
        }
        buffer = grouped_lines.pop().unwrap_or_default();
    }

    if !buffer.is_empty() {
        lines.push(render_line(&buffer, true));
    }

    lines
}
