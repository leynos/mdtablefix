//! Inline wrapping helpers that keep code spans intact.
//!
//! These functions operate on token streams so `wrap_text` can preserve
//! inline code, links, and trailing punctuation without reimplementing the
//! grouping logic in multiple places.

mod postprocess;

use std::ops::Range;

use postprocess::{merge_whitespace_only_lines, rebalance_atomic_tails};
use textwrap::{core::Fragment, wrap_algorithms::wrap_first_fit};
use unicode_width::UnicodeWidthStr;

use super::tokenize;

/// Describes the Markdown role of a grouped token span used during
/// span-coupling decisions.
#[derive(Copy, Clone, PartialEq, Eq)]
enum SpanKind {
    /// Ordinary prose text with no special Markdown role.
    General,
    /// An inline code span delimited by backticks.
    Code,
    /// A Markdown inline link or image.
    Link,
}

/// Returns `true` when `c` is trailing punctuation that should stay attached
/// to the preceding code span or link rather than breaking to a new line.
#[inline]
fn is_trailing_punct(c: char) -> bool {
    // ASCII closers + common Unicode closers and word-final punctuation
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '"' | '\''
    ) || "…—–»›）］】》」』、。，：；！？".'".contains(c)
}

/// Returns `true` when `token` looks like a complete Markdown inline link or
/// image reference (`[text](url)` or `![alt](url)`).
fn looks_like_link(token: &str) -> bool {
    (token.starts_with('[') || token.starts_with("!["))
        && token.contains("](")
        && token.ends_with(')')
}

/// Returns `true` when every character in `token` is a Unicode whitespace
/// character.
fn is_whitespace_token(token: &str) -> bool { token.chars().all(char::is_whitespace) }

/// Returns `true` when `token` is a complete backtick-delimited inline code span.
fn is_inline_code_token(token: &str) -> bool { token.starts_with('`') && token.ends_with('`') }

/// Advances `j` past any trailing-punctuation tokens, accumulating widths into
/// `*width`, and returns the updated index.
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

/// Merges a backtick-delimited code span starting at `i` into a single span,
/// accumulating display width and returning the index beyond the span.
/// Unmatched opening backticks extend to the end of the token list.
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

/// Groups consecutive tokens starting at `start` into a single logical span
/// whose fragments should not be broken across lines.
///
/// Returns `(end, width)` where `end` is the exclusive index of the last token
/// in the span and `width` is the total display-column width of all tokens
/// from `start` to `end`.
///
/// Inline code spans (including single-backtick openers matched to a closer),
/// Markdown links, and images are kept atomic. Trailing punctuation is absorbed
/// into the preceding span. Whitespace is absorbed only when
/// `should_couple_whitespace` indicates the surrounding context requires it.
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

/// Appends a single-character punctuation token to the last line in `lines`
/// when that line ends with an inline code span and the current buffer is
/// empty.
///
/// Returns `true` when the token was consumed, `false` when the caller should
/// continue processing it normally.
///
/// This function is only compiled for tests because the production path now
/// uses fragment-level classification instead of string-level post-processing.
#[cfg(test)]
pub(super) fn attach_punctuation_to_previous_line(
    lines: &mut [String],
    current: &str,
    token: &str,
) -> bool {
    if !current.is_empty() || token.len() != 1 || !".?!,:;".contains(token) {
        return false;
    }

    let Some(last_line) = lines.last_mut() else {
        return false;
    };

    if last_line.trim_end().ends_with('`') {
        last_line.push_str(token);
        return true;
    }

    false
}

/// Classifies an inline fragment for post-wrap normalisation and
/// `textwrap::core::Fragment` width reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FragmentKind {
    /// A fragment composed entirely of whitespace characters.
    Whitespace,
    /// A backtick-delimited inline code span, optionally followed by trailing
    /// punctuation.
    InlineCode,
    /// A Markdown inline link or image reference.
    Link,
    /// Ordinary prose text that does not match any of the above categories.
    Plain,
}

/// A Markdown-aware inline fragment that implements
/// `textwrap::core::Fragment`.
///
/// Each fragment holds the rendered text, its precomputed display-column
/// width, and a [`FragmentKind`] that drives post-wrap normalisation decisions
/// in `merge_whitespace_only_lines` and `rebalance_atomic_tails`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InlineFragment {
    /// The rendered text of this fragment as it will appear in output.
    text: String,
    /// Display-column width of `text` computed with `unicode-width`.
    width: usize,
    /// The Markdown role of this fragment.
    kind: FragmentKind,
}

impl InlineFragment {
    /// Constructs a new `InlineFragment` from `text`, computing the display
    /// width and classifying the kind automatically.
    fn new(text: String) -> Self {
        let width = UnicodeWidthStr::width(text.as_str());
        let kind = classify_fragment(text.as_str());
        Self { text, width, kind }
    }

    /// Returns `true` when this fragment is a whitespace-only fragment.
    fn is_whitespace(&self) -> bool { self.kind == FragmentKind::Whitespace }

    /// Returns `true` when this fragment is an atomic inline element (inline
    /// code span or Markdown link) that must not be split across lines.
    fn is_atomic(&self) -> bool {
        matches!(self.kind, FragmentKind::InlineCode | FragmentKind::Link)
    }

    /// Returns `true` when this fragment is ordinary plain-text prose.
    fn is_plain(&self) -> bool { self.kind == FragmentKind::Plain }
}

impl Fragment for InlineFragment {
    /// Display-column width as `f64` for `textwrap::wrap_algorithms`.
    fn width(&self) -> f64 { width_as_f64(self.width) }

    /// Zero — whitespace is handled by fragment classification, not `textwrap`.
    fn whitespace_width(&self) -> f64 { 0.0 }

    /// Zero — no penalty is applied at any fragment boundary.
    fn penalty_width(&self) -> f64 { 0.0 }
}

/// Converts a `usize` display width to `f64`, clamping at `u32::MAX` to avoid
/// precision loss for pathologically long tokens.
fn width_as_f64(width: usize) -> f64 { f64::from(u32::try_from(width).unwrap_or(u32::MAX)) }

/// Determines the [`FragmentKind`] for a rendered fragment text.
///
/// Whitespace-only text maps to `Whitespace`. Text whose non-punctuation core
/// is a backtick-delimited code span maps to `InlineCode`. Text whose core is
/// a Markdown link or image maps to `Link`. Everything else maps to `Plain`.
fn classify_fragment(text: &str) -> FragmentKind {
    if is_whitespace_token(text) {
        return FragmentKind::Whitespace;
    }
    let trimmed = text.trim_end_matches(is_trailing_punct);
    if is_inline_code_token(trimmed) {
        FragmentKind::InlineCode
    } else if looks_like_link(trimmed) {
        FragmentKind::Link
    } else {
        FragmentKind::Plain
    }
}

/// Appends tokens in `span` to `text`. When a punctuation token immediately
/// follows a backtick-terminated code span, trailing whitespace is trimmed from
/// `text` first so the punctuation attaches directly to the closing backtick.
fn push_span_text(text: &mut String, tokens: &[String], span: Range<usize>) {
    for token in &tokens[span] {
        if token.len() == 1 && ".?!,:;".contains(token) && text.trim_end().ends_with('`') {
            text.truncate(text.trim_end_matches(char::is_whitespace).len());
        }
        text.push_str(token);
    }
}

/// Converts a token list into a `Vec<InlineFragment>` by grouping consecutive
/// tokens into logical spans via `determine_token_span`. Whitespace-only spans
/// are joined directly; other spans pass through `push_span_text`.
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

/// Renders a line of `InlineFragment` slices into a `String`.
///
/// Non-final lines have a single trailing space removed to avoid unintended
/// hard line breaks (two trailing spaces are preserved). The final line is
/// always rendered as-is so intentional trailing spaces survive wrapping.
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

/// Wraps `text` to at most `width` display columns while preserving inline
/// code spans, Markdown links, and trailing punctuation as atomic units.
///
/// The function tokenises `text` with `tokenize::segment_inline`, groups the
/// tokens into `InlineFragment` values, and feeds them incrementally to
/// `textwrap::wrap_algorithms::wrap_first_fit`. After each fragment is added,
/// the current fit is post-processed by `merge_whitespace_only_lines` and
/// `rebalance_atomic_tails` to normalise separator lines and balance trailing
/// fragments. Completed lines are emitted and only the trailing incomplete
/// line is retained in the buffer for the next iteration.
///
/// Returns an empty `Vec` when `text` is empty.
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
