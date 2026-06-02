//! Span-grouping helpers for inline token streams.
//!
//! These functions extend grouped spans over punctuation, whitespace, adjacent
//! footnote markers, and chained inline code or link tokens during
//! `determine_token_span`.

use unicode_width::UnicodeWidthStr;

use super::predicates::{
    is_inline_code_token,
    is_opening_punct,
    is_trailing_punct,
    is_trailing_punctuation_token,
    looks_like_footnote_ref,
    looks_like_link,
};

/// Marks how a grouped token span should behave during wrapping.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in crate::wrap::inline) enum SpanKind {
    /// Treat the span as ordinary prose.
    General,
    /// Treat the span as an inline code sequence.
    Code,
    /// Treat the span as a Markdown link or image link.
    Link,
    /// Treat the span as a GitHub Flavoured Markdown footnote reference.
    FootnoteRef,
}

/// Extends a grouped span over trailing punctuation tokens and updates `width`.
pub(in crate::wrap::inline) fn extend_punctuation(
    tokens: &[String],
    mut j: usize,
    width: &mut usize,
) -> usize {
    while j < tokens.len() && is_trailing_punctuation_token(&tokens[j]) {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
    }
    j
}

/// Decide whether whitespace between grouped tokens should stay attached to the
/// current span.
pub(in crate::wrap::inline) fn should_couple_whitespace(
    kind: SpanKind,
    next_token: Option<&String>,
) -> bool {
    match (kind, next_token) {
        (SpanKind::Link, Some(next))
            if looks_like_link(next)
                || is_inline_code_token(next)
                || is_trailing_punctuation_token(next) =>
        {
            true
        }
        (SpanKind::Code, Some(next)) if is_trailing_punctuation_token(next) => true,
        _ => false,
    }
}

/// Merges a backtick-opened code span into one grouped span and updates
/// `width`.
#[inline]
pub(in crate::wrap::inline) fn merge_code_span(
    tokens: &[String],
    i: usize,
    width: &mut usize,
) -> usize {
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
pub(in crate::wrap::inline) fn absorb_token_and_trailing_punctuation(
    tokens: &[String],
    end: usize,
    width: &mut usize,
) -> usize {
    *width += UnicodeWidthStr::width(tokens[end].as_str());
    extend_punctuation(tokens, end + 1, width)
}

/// Couples an opener-followed inline link, such as `([1](url))`, into the
/// current span.
pub(in crate::wrap::inline) fn try_couple_inline_link_after_opener(
    tokens: &[String],
    end: usize,
    width: &mut usize,
) -> Option<(SpanKind, usize)> {
    let opener = tokens.get(end)?;
    let link = tokens.get(end + 1)?;
    if !opener.chars().all(is_opening_punct) || !looks_like_link(link) {
        return None;
    }

    *width += UnicodeWidthStr::width(opener.as_str());
    *width += UnicodeWidthStr::width(link.as_str());
    Some((SpanKind::Link, extend_punctuation(tokens, end + 2, width)))
}

/// Couples an adjacent footnote reference into the current span when appropriate.
pub(in crate::wrap::inline) fn try_couple_footnote_reference(
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
