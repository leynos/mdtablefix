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

use unicode_width::UnicodeWidthStr;

use super::{
    predicates::{
        ends_with_hyphen_prefix,
        is_inline_code_token,
        is_opening_punct,
        is_trailing_punctuation_token,
        is_whitespace_token,
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

fn is_code_token(token: &str) -> bool {
    is_inline_code_token(token) || has_inline_code_structure(token)
}

fn initial_token_span(
    tokens: &[String],
    start: usize,
    observer: &mut ObserverHandle<'_>,
) -> (usize, usize, SpanKind) {
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
    } else if looks_like_footnote_ref(&tokens[start], observer) {
        kind = SpanKind::FootnoteRef;
        end = extend_punctuation(tokens, end, &mut width);
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

/// Reports whether whitespace before a colon-suffixed footnote reference was
/// coupled into the current span.
///
/// Nothing is reported unless the next token really is a footnote reference, so
/// the event describes a grouping decision rather than every whitespace run.
/// The probe passes `&mut None`: it only selects whether to report, and
/// forwarding the observer would emit a `FootnoteRefChecked` event for it.
fn emit_whitespace_footnote_coupling(
    kind: SpanKind,
    next_token: Option<&String>,
    following_token: Option<&String>,
    coupled: bool,
    observer: &mut ObserverHandle<'_>,
) {
    let Some(token) = next_token.filter(|token| looks_like_footnote_ref(token, &mut None)) else {
        return;
    };
    if let Some(observer) = observer.as_deref_mut() {
        observer.observe(Event::WhitespaceFootnoteCoupling {
            kind,
            token,
            has_following_colon: following_token.is_some_and(|following| following == ":"),
            coupled,
        });
    }
}

/// Reports whether an adjacent footnote reference was coupled into the current
/// span, and the context that decided it.
fn emit_footnote_reference_coupling(
    tokens: &[String],
    end: usize,
    kind: SpanKind,
    coupled: bool,
    observer: &mut ObserverHandle<'_>,
) {
    let Some(token) = tokens
        .get(end)
        .filter(|token| looks_like_footnote_ref(token, &mut None))
    else {
        return;
    };
    let follows_space_before_colon = end
        .checked_sub(1)
        .and_then(|previous| tokens.get(previous))
        .is_some_and(|previous| previous.chars().all(char::is_whitespace))
        && tokens
            .get(end + 1)
            .is_some_and(|following| following == ":");
    if let Some(observer) = observer.as_deref_mut() {
        observer.observe(Event::FootnoteReferenceCoupling {
            kind,
            token,
            follows_space_before_colon,
            coupled,
        });
    }
}

// `pub(in crate::wrap::inline)` so the span-helper tracing tests can group
// tokens with a live observer attached; production callers reach this through
// `wrap_preserving_code_observed`.
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

    let (mut end, mut width, mut kind) = initial_token_span(tokens, start, observer);

    while end < tokens.len() {
        let token = &tokens[end];
        if is_whitespace_token(token) {
            let next_token = tokens.get(end + 1);
            let following_token = tokens.get(end + 2);
            let should_couple =
                should_couple_whitespace(kind, next_token, following_token, observer);
            emit_whitespace_footnote_coupling(
                kind,
                next_token,
                following_token,
                should_couple,
                observer,
            );
            if should_couple {
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
        if let Some((next_kind, next_end)) =
            try_couple_inline_link_after_opener(tokens, end, &mut width)
        {
            kind = next_kind;
            end = next_end;
            continue;
        }

        // Footnote markers must be coupled before consecutive link/code chaining;
        // otherwise `[^N]` stays a separate wrap token even when punctuation is
        // already attached to the preceding atomic span.
        let footnote_coupling =
            try_couple_footnote_reference(tokens, end, kind, &mut width, observer);
        emit_footnote_reference_coupling(tokens, end, kind, footnote_coupling.is_some(), observer);
        if let Some((next_kind, next_end)) = footnote_coupling {
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
