//! Select atomic Markdown token spans before inline line fitting.
//!
//! This module classifies the first token and then extends it across attached
//! punctuation, references, links, and inline code without splitting UTF-8 text.

use tracing::trace;
use unicode_width::UnicodeWidthStr;

use super::{
    is_code_token,
    predicates::{
        ends_with_hyphen_prefix,
        is_opening_punct,
        is_trailing_punctuation_token,
        is_whitespace_token,
        looks_like_bracketed_reference,
        looks_like_footnote_ref,
        looks_like_link,
    },
    span_helpers::{
        SpanKind,
        absorb_token_and_trailing_punctuation,
        date_token_span,
        extend_punctuation,
        merge_code_span,
        should_couple_whitespace,
        try_couple_bracketed_reference,
        try_couple_footnote_reference,
        try_couple_inline_link_after_opener,
    },
    tracing_events::{emit_footnote_reference_coupling, emit_whitespace_footnote_coupling},
};

/// Build the first atomic span at `start`, including punctuation and attached
/// Markdown constructs that cannot be split across a line boundary.
///
/// Opening punctuation, hyphen prefixes, code spans, links, and footnote
/// references are coupled before the general continuation loop runs. The
/// returned width is the Unicode display width of that complete candidate.
fn initial_token_span(tokens: &[String], start: usize) -> (usize, usize, SpanKind) {
    let Some(first) = tokens.get(start) else {
        return (start, 0, SpanKind::General);
    };
    let mut span = SpanCursor {
        end: start + 1,
        width: UnicodeWidthStr::width(first.as_str()),
        kind: SpanKind::General,
    };
    couple_opening(tokens, start, first, &mut span);
    couple_hyphen_prefix(tokens, first, &mut span);
    classify_first(tokens, start, first, &mut span);
    (span.end, span.width, span.kind)
}

/// Keep opening punctuation with a following Markdown construct.
fn couple_opening(tokens: &[String], start: usize, first: &str, span: &mut SpanCursor) {
    // A lone opener at the end of a line would detach from its code or link.
    if first.chars().all(is_opening_punct)
        && let Some(next) = tokens.get(start + 1)
        && let Some(kind) = opening_coupling_kind(first, next)
    {
        span.kind = kind;
        span.end += 1;
        span.width += UnicodeWidthStr::width(next.as_str());
        span.end = extend_punctuation(tokens, span.end, &mut span.width);
    }
}

/// Keep a hyphen prefix attached to the code span immediately after it.
fn couple_hyphen_prefix(tokens: &[String], first: &str, span: &mut SpanCursor) {
    if span.kind == SpanKind::General
        && ends_with_hyphen_prefix(first)
        && let Some(next) = tokens.get(span.end)
        && is_code_token(next)
    {
        span.kind = SpanKind::Code;
        span.width += UnicodeWidthStr::width(next.as_str());
        span.end += 1;
        span.end = extend_punctuation(tokens, span.end, &mut span.width);
    }
}

/// Classify a code, link, or footnote token and attach trailing punctuation.
fn classify_first(tokens: &[String], start: usize, first: &str, span: &mut SpanCursor) {
    if first == "`" {
        span.kind = SpanKind::Code;
        span.end = merge_code_span(tokens, start, &mut span.width);
    } else if is_code_token(first) {
        span.kind = SpanKind::Code;
        span.end = extend_punctuation(tokens, span.end, &mut span.width);
    } else if looks_like_link(first) {
        span.kind = SpanKind::Link;
        span.end = extend_punctuation(tokens, span.end, &mut span.width);
    } else if looks_like_footnote_ref(first) {
        span.kind = SpanKind::FootnoteRef;
        span.end = extend_punctuation(tokens, span.end, &mut span.width);
    }
}

/// Classify an atomic token that must stay with its opening punctuation.
fn opening_coupling_kind(first: &str, next: &str) -> Option<SpanKind> {
    if is_code_token(next) {
        Some(SpanKind::Code)
    } else if looks_like_link(next) {
        Some(SpanKind::Link)
    } else if first == "[" && looks_like_bracketed_reference(next) {
        // Only `[` introduces a bare numeric reference.
        Some(SpanKind::BracketedRef)
    } else {
        None
    }
}

/// Finds the next logical token group starting at `start`.
///
/// `tokens` is the segmented inline token stream and `start` is the first
/// token in the next candidate group. The return value is `(end, width)`,
/// where `end` is the exclusive end index of the grouped inline code span,
/// link, or plain fragment, and `width` is its Unicode display width. An
/// empty or out-of-range stream returns a zero-width span at `start`.
pub(in crate::wrap) fn determine_token_span(tokens: &[String], start: usize) -> (usize, usize) {
    if tokens.get(start).is_none() {
        return (start, 0);
    }
    if let Some((end, width)) = date_token_span(tokens, start) {
        // Keep the diagnostic target stable when span selection lives here.
        trace!(
            target: "mdtablefix::wrap::inline",
            start,
            end, width, "determine_token_span grouped date sequence"
        );
        return (end, width);
    }

    let (end, width, kind) = initial_token_span(tokens, start);
    let mut span = SpanCursor { end, width, kind };
    while advance_span(tokens, &mut span) {}
    (span.end, span.width)
}

/// Position, width, and classification of the span under construction.
struct SpanCursor {
    /// Exclusive end of the selected token span.
    end: usize,
    /// Unicode display width of the selected token span.
    width: usize,
    /// Markdown construct currently being extended.
    kind: SpanKind,
}

/// Consume the next token only when it belongs to the current atomic span.
fn advance_span(tokens: &[String], span: &mut SpanCursor) -> bool {
    let Some(token) = tokens.get(span.end) else {
        return false;
    };
    if is_whitespace_token(token) {
        return couple_whitespace(tokens, span, token);
    }
    if is_trailing_punctuation_token(token) {
        return couple_trailing_punctuation(span, token);
    }
    couple_attached_reference(tokens, span) || couple_adjacent_atom(tokens, span, token)
}

/// Keep whitespace only when the following token extends an atomic group.
fn couple_whitespace(tokens: &[String], span: &mut SpanCursor, token: &str) -> bool {
    let next_token = tokens.get(span.end + 1);
    let following_token = tokens.get(span.end + 2);
    let should_couple = should_couple_whitespace(span.kind, next_token, following_token);
    emit_whitespace_footnote_coupling(span.kind, next_token, following_token, should_couple);
    if should_couple {
        span.width += UnicodeWidthStr::width(token);
        span.end += 1;
    }
    should_couple
}

/// Attach punctuation after code, links, and footnote references.
fn couple_trailing_punctuation(span: &mut SpanCursor, token: &str) -> bool {
    let should_couple = matches!(
        span.kind,
        SpanKind::Code | SpanKind::Link | SpanKind::FootnoteRef
    );
    if should_couple {
        span.width += UnicodeWidthStr::width(token);
        span.end += 1;
    }
    should_couple
}

/// Attach a link, bare bracket reference, or footnote marker to its opener.
fn couple_attached_reference(tokens: &[String], span: &mut SpanCursor) -> bool {
    if let Some((kind, end)) =
        try_couple_inline_link_after_opener(tokens, span.end, &mut span.width)
    {
        span.kind = kind;
        span.end = end;
        return true;
    }

    // Bare bracket references need their opener to avoid stranding `[`.
    if let Some((kind, end)) = try_couple_bracketed_reference(tokens, span.end, &mut span.width) {
        span.kind = kind;
        span.end = end;
        return true;
    }

    let coupling = try_couple_footnote_reference(tokens, span.end, span.kind, &mut span.width);
    emit_footnote_reference_coupling(tokens, span.end, span.kind, coupling.is_some());
    if let Some((kind, end)) = coupling {
        span.kind = kind;
        span.end = end;
        return true;
    }
    false
}

/// Chain adjacent links or code spans without introducing a break point.
fn couple_adjacent_atom(tokens: &[String], span: &mut SpanCursor, token: &str) -> bool {
    let is_same_kind = (span.kind == SpanKind::Link && looks_like_link(token))
        || (span.kind == SpanKind::Code && is_code_token(token));
    if is_same_kind {
        span.end = absorb_token_and_trailing_punctuation(tokens, span.end, &mut span.width);
    }
    is_same_kind
}

#[cfg(test)]
mod tests {
    //! Boundary and Unicode checks for the span-selection cursor.

    use rstest::rstest;

    use super::determine_token_span;

    #[rstest]
    #[case::empty(&[], 0, (0, 0))]
    #[case::unicode(&["表"], 0, (1, 2))]
    #[case::at_end(&["表"], 1, (1, 0))]
    #[case::past_end(&["表"], usize::MAX, (usize::MAX, 0))]
    fn span_selection_handles_stream_boundaries_and_display_width(
        #[case] words: &[&str],
        #[case] start: usize,
        #[case] expected: (usize, usize),
    ) {
        let tokens = words.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(determine_token_span(&tokens, start), expected);
    }
}
