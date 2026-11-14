//! Token-span grouping for inline wrapping.
//!
//! These helpers decide where one logical wrap token ends and the next begins,
//! coupling opening punctuation, inline code, links, dates, and GFM footnote
//! references into atomic spans. They also report the grouping decisions that
//! carry diagnostic value as domain `Event` values, leaving the translation to
//! `tracing` to the adapter.
//!
//! `wrapping` consumes the spans produced here and turns them into fitted
//! lines; the two are separate modules so each stays a readable size.

use std::ops::Range;

use unicode_width::UnicodeWidthStr;

use super::{
    coupling_report::{emit_footnote_reference_coupling, emit_whitespace_footnote_coupling},
    predicates::{
        is_inline_code_token,
        is_opening_punct,
        is_trailing_punctuation_token,
        is_whitespace_token,
        looks_like_bracketed_reference,
        looks_like_footnote_ref,
        looks_like_link,
    },
    span_helpers::{
        SpanCursor,
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
};
use crate::wrap::observer::{Event, ObserverHandle};

/// Returns whether `token` begins with a matched inline code fence, optionally
/// followed by a non-whitespace suffix such as an inflectional affix.
fn has_inline_code_structure(token: &str) -> bool {
    super::fragment::has_inline_code_structure(token)
}

/// Returns whether `token` is an inline code span, including the bare and
/// suffixed forms `has_inline_code_structure` recognizes.
fn is_code_token(token: &str) -> bool {
    is_inline_code_token(token) || has_inline_code_structure(token)
}

/// A coupling decision about the token at `end`, ready to be reported.
#[derive(Clone, Copy)]
pub(super) struct CouplingReport<'a> {
    /// The token stream the decision was made over.
    pub(super) tokens: &'a [String],
    /// The index of the token the decision concerns.
    pub(super) end: usize,
    /// The classification of the span so far.
    pub(super) kind: SpanKind,
    /// Whether the token was coupled into the span.
    pub(super) coupled: bool,
}

/// Returns the span kind an opening punctuation token couples to, if any.
///
/// Only `[` introduces a bare bracket reference, so other openers stay ordinary
/// prose even when the token that follows closes like a reference.
fn opening_coupling_kind(opener: &str, next: &str) -> Option<SpanKind> {
    if is_code_token(next) {
        Some(SpanKind::Code)
    } else if looks_like_link(next) {
        Some(SpanKind::Link)
    } else if opener == "[" && looks_like_bracketed_reference(next) {
        Some(SpanKind::BracketedRef)
    } else {
        None
    }
}

/// Couples an opening-punctuation token to the atomic span that follows it.
///
/// Forward-coupling keeps wrapping from leaving a lone `(` at the end of a line
/// before inline code or a link. Returns the new exclusive end and the span
/// kind when coupling happened.
fn couple_opening_punctuation(
    tokens: &[String],
    start: usize,
    width: &mut usize,
) -> Option<(usize, SpanKind)> {
    if !tokens[start].chars().all(is_opening_punct) {
        return None;
    }
    let next = tokens.get(start + 1)?;
    let kind = opening_coupling_kind(&tokens[start], next)?;
    *width += UnicodeWidthStr::width(next.as_str());
    Some((extend_punctuation(tokens, start + 2, width), kind))
}

/// Couples an adjacent unspaced token to the inline code span that follows it.
///
/// A wrap boundary between two unspaced tokens becomes a space when the
/// formatter rereads the paragraph, so the seam has to stay atomic. The rule is
/// defined over the token class rather than a spelling: any non-whitespace
/// token binds to a following code span, which covers `` pre-`code` `` as well
/// as the emphasis and link seams a hyphen test missed. Whitespace is
/// deliberately still a break opportunity.
///
/// Returns the new exclusive end when coupling happened.
fn couple_unspaced_before_code(
    tokens: &[String],
    start: usize,
    end: usize,
    width: &mut usize,
) -> Option<usize> {
    let next = tokens.get(end)?;
    if is_whitespace_token(&tokens[start]) || !is_code_token(next) {
        return None;
    }
    *width += UnicodeWidthStr::width(next.as_str());
    Some(extend_punctuation(tokens, end + 1, width))
}

/// Returns the span kind implied by a single token, ignoring punctuation.
fn first_token_kind(token: &str, observer: &mut ObserverHandle<'_>) -> SpanKind {
    if is_code_token(token) {
        SpanKind::Code
    } else if looks_like_link(token) {
        SpanKind::Link
    } else if looks_like_footnote_ref(token, observer) {
        SpanKind::FootnoteRef
    } else {
        SpanKind::General
    }
}

/// Classifies the first token of a span and extends it over punctuation.
///
/// Returns `None` when the token is ordinary prose, which leaves the span as
/// the single token it started as.
fn classify_first_token(
    tokens: &[String],
    span: Range<usize>,
    width: &mut usize,
    observer: &mut ObserverHandle<'_>,
) -> Option<(usize, SpanKind)> {
    if tokens[span.start] == "`" {
        return Some((merge_code_span(tokens, span.start, width), SpanKind::Code));
    }
    let kind = first_token_kind(&tokens[span.start], observer);
    if kind == SpanKind::General {
        return None;
    }
    Some((extend_punctuation(tokens, span.end, width), kind))
}

/// Build the first atomic span at `start`, including punctuation and attached
/// Markdown constructs that cannot be split across a line boundary.
///
/// Opening punctuation, unspaced runs before a code span, code spans, links,
/// bare bracket references, and footnote references are coupled before the
/// general continuation loop runs. The returned width is the Unicode display
/// width of that complete candidate.
fn initial_token_span(
    tokens: &[String],
    start: usize,
    observer: &mut ObserverHandle<'_>,
) -> (usize, usize, SpanKind) {
    let mut width = UnicodeWidthStr::width(tokens[start].as_str());
    let mut end = start + 1;
    let mut kind = SpanKind::General;

    if let Some((coupled_end, coupled_kind)) = couple_opening_punctuation(tokens, start, &mut width)
    {
        end = coupled_end;
        kind = coupled_kind;
    } else if let Some(coupled_end) = couple_unspaced_before_code(tokens, start, end, &mut width) {
        end = coupled_end;
        kind = SpanKind::Code;
    }

    if let Some((classified_end, classified_kind)) =
        classify_first_token(tokens, start..end, &mut width, observer)
    {
        end = classified_end;
        kind = classified_kind;
    }

    (end, width, kind)
}

/// Finds the next logical token group starting at `start`.
///
/// `tokens` is the segmented inline token stream and `start` is the first
/// token in the next candidate group. The return value is `(end, width)`,
/// where `end` is the exclusive end index of the grouped inline code span,
/// link, or plain fragment, and `width` is its Unicode display width. This
/// helper assumes `start < tokens.len()` and will panic if called out of
/// bounds.
#[cfg(test)]
pub(in crate::wrap) fn determine_token_span(tokens: &[String], start: usize) -> (usize, usize) {
    determine_token_span_observed(tokens, start, &mut None)
}

/// Couples a whitespace run to the span when the following token requires it.
///
/// Returns `false` when the run ends the span, which stops grouping.
fn couple_whitespace(cursor: &mut SpanCursor<'_>, observer: &mut ObserverHandle<'_>) -> bool {
    let next_token = cursor.tokens.get(cursor.end + 1);
    let following_token = cursor.tokens.get(cursor.end + 2);
    let should_couple =
        should_couple_whitespace(cursor.kind, next_token, following_token, observer);
    emit_whitespace_footnote_coupling(
        CouplingReport {
            tokens: cursor.tokens,
            end: cursor.end,
            kind: cursor.kind,
            coupled: should_couple,
        },
        observer,
    );
    if !should_couple {
        return false;
    }
    cursor.width += UnicodeWidthStr::width(cursor.tokens[cursor.end].as_str());
    cursor.end += 1;
    true
}

/// Absorbs a trailing punctuation token when the span is atomic.
///
/// Returns `false` when the span is plain prose, which stops grouping.
fn absorb_trailing_punctuation(cursor: &mut SpanCursor<'_>) -> bool {
    if !matches!(
        cursor.kind,
        SpanKind::Code | SpanKind::Link | SpanKind::FootnoteRef
    ) {
        return false;
    }
    cursor.width += UnicodeWidthStr::width(cursor.tokens[cursor.end].as_str());
    cursor.end += 1;
    true
}

/// Couples an adjacent link, bracket reference, or footnote marker.
///
/// Returns `false` when the token starts a new span instead.
fn couple_adjacent_atomic(cursor: &mut SpanCursor<'_>, observer: &mut ObserverHandle<'_>) -> bool {
    if try_couple_inline_link_after_opener(cursor) {
        return true;
    }

    // A bare bracket reference couples to its opener for the same reason
    // footnote markers do: the opener can follow an atomic span directly, and
    // leaving `[` in the preceding span would strand it at a line end.
    if try_couple_bracketed_reference(cursor) {
        return true;
    }

    // Footnote markers must be coupled before consecutive link/code chaining;
    // otherwise `[^N]` stays a separate wrap token even when punctuation is
    // already attached to the preceding atomic span. The report describes the
    // decision, so it is built before the coupling moves the cursor.
    let report = CouplingReport {
        tokens: cursor.tokens,
        end: cursor.end,
        kind: cursor.kind,
        coupled: false,
    };
    let coupled = try_couple_footnote_reference(cursor, observer);
    emit_footnote_reference_coupling(CouplingReport { coupled, ..report }, observer);
    coupled
}

/// Absorbs a chained link or code token of the same kind as the span.
///
/// Returns `false` when the token does not continue the span.
fn absorb_chained_atomic(cursor: &mut SpanCursor<'_>) -> bool {
    let token = &cursor.tokens[cursor.end];
    if cursor.kind == SpanKind::Link && looks_like_link(token) {
        absorb_token_and_trailing_punctuation(cursor);
        return true;
    }
    if cursor.kind == SpanKind::Code && is_code_token(token) {
        absorb_token_and_trailing_punctuation(cursor);
        return true;
    }
    false
}

/// Extends the span by one grouping decision.
///
/// Returns `false` when the span cannot grow further, which ends the grouping
/// loop.
fn extend_span(cursor: &mut SpanCursor<'_>, observer: &mut ObserverHandle<'_>) -> bool {
    let token = &cursor.tokens[cursor.end];
    if is_whitespace_token(token) {
        return couple_whitespace(cursor, observer);
    }
    if is_trailing_punctuation_token(token) {
        return absorb_trailing_punctuation(cursor);
    }
    if couple_adjacent_atomic(cursor, observer) {
        return true;
    }
    absorb_chained_atomic(cursor)
}

/// Finds the next logical token group starting at `start`, reporting the
/// grouping decisions that carry diagnostic value.
///
/// `tokens` is the segmented inline token stream and `start` is the first token
/// in the next candidate group. The return value is `(end, width)`, where `end`
/// is the exclusive end index of the grouped inline code span, link, date
/// sequence, or plain fragment, and `width` is its Unicode display width.
///
/// Like [`determine_token_span`], this assumes `start < tokens.len()` and
/// panics if called out of bounds.
///
/// Events are reported only when `observer` is `Some`; with `None` the grouping
/// is identical and nothing is emitted, because the observer is a diagnostics
/// channel and never participates in layout.
///
/// `pub(in crate::wrap::inline)` so the span-helper tracing tests can group
/// tokens with a live observer attached; production callers reach this through
/// `wrap_preserving_code_observed`.
pub(in crate::wrap::inline) fn determine_token_span_observed(
    tokens: &[String],
    start: usize,
    observer: &mut ObserverHandle<'_>,
) -> (usize, usize) {
    if let Some((end, width)) = date_token_span(tokens, start, observer) {
        if let Some(observer) = observer.as_deref_mut() {
            observer.observe(Event::DateSequenceGrouped { start, end, width });
        }
        return (end, width);
    }

    let (end, width, kind) = initial_token_span(tokens, start, observer);
    let mut cursor = SpanCursor {
        tokens,
        end,
        width,
        kind,
    };
    while cursor.end < tokens.len() && extend_span(&mut cursor, observer) {}
    (cursor.end, cursor.width)
}
