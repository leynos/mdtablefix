//! `tracing` integration for inline-classification domain events.

use tracing::{debug, trace};

use super::observer::{Event, Observer, SpanKind};

/// Records the outcome of weighing whitespace before a footnote reference.
///
/// Split out of `observe` so each grouping-boundary translation stays readable
/// and the match arm keeps to one statement.
fn log_whitespace_footnote_coupling(
    kind: SpanKind,
    token: &str,
    has_following_colon: bool,
    coupled: bool,
) {
    let token_length = token.chars().count();
    if coupled {
        debug!(
            span_kind = ?kind,
            token_length,
            has_following_colon,
            "coupled whitespace before colon-suffixed footnote reference"
        );
    } else {
        debug!(
            span_kind = ?kind,
            token_length,
            has_following_colon,
            error_category = "footnote_colon_whitespace_coupling_declined",
            "declined whitespace coupling before footnote reference"
        );
    }
}

/// Records the outcome of weighing an adjacent footnote reference.
fn log_footnote_reference_coupling(
    kind: SpanKind,
    token: &str,
    follows_space_before_colon: bool,
    coupled: bool,
) {
    let token_length = token.chars().count();
    if coupled && follows_space_before_colon {
        debug!(
            span_kind = ?kind,
            token_length,
            has_following_colon = true,
            "coupled colon-suffixed footnote reference after whitespace"
        );
    } else if !coupled {
        debug!(
            span_kind = ?kind,
            token_length,
            follows_space_before_colon,
            error_category = "footnote_coupling_context_mismatch",
            "declined footnote reference coupling"
        );
    }
}

/// Translates domain events into the crate's existing `tracing` events.
///
/// The adapter owns every vendor-specific concern: the `tracing` level gate and
/// any derived value that costs more than a copy (Unicode length counts).
/// Domain emitters hand over only borrowed slices, so a disabled subscriber
/// pays nothing beyond the branch below.
///
/// Every field emitted here is content-free metadata. Borrowed token text is
/// used only to derive bounded values such as `token_length`; the text itself
/// never reaches a subscriber.
pub(crate) struct TracingObserver;

impl Observer for TracingObserver {
    fn observe(&mut self, event: Event<'_>) {
        match event {
            Event::FootnoteReferenceParsed { token }
                if tracing::enabled!(tracing::Level::DEBUG) =>
            {
                debug!(
                    token_length = token.chars().count(),
                    "footnote reference parsed"
                );
            }
            Event::LinkOrImageParsed { token, is_image }
                if tracing::enabled!(tracing::Level::DEBUG) =>
            {
                debug!(
                    token_length = token.chars().count(),
                    is_image, "link or image parsed"
                );
            }
            Event::FootnoteEndNotFound { start, reason }
                if tracing::enabled!(tracing::Level::TRACE) =>
            {
                trace!(start, reason, "footnote end not found");
            }
            Event::FootnoteLabelRecognized { start, end, token }
                if tracing::enabled!(tracing::Level::TRACE) =>
            {
                trace!(
                    start,
                    end,
                    token_length = token.chars().count(),
                    "footnote label span recognized"
                );
            }
            Event::FootnoteRefChecked { token, result }
                if tracing::enabled!(tracing::Level::TRACE) =>
            {
                trace!(
                    token_length = token.chars().count(),
                    result, "footnote reference checked"
                );
            }
            Event::DateSequenceMatched {
                start,
                end,
                pattern,
            } if tracing::enabled!(tracing::Level::DEBUG) => {
                debug!(start, end, pattern, "matched date sequence");
            }
            Event::DateSequenceGrouped { start, end, width }
                if tracing::enabled!(tracing::Level::TRACE) =>
            {
                trace!(
                    start,
                    end, width, "determine_token_span grouped date sequence"
                );
            }
            Event::WhitespaceFootnoteCoupling {
                kind,
                token,
                has_following_colon,
                coupled,
            } if tracing::enabled!(tracing::Level::DEBUG) => {
                log_whitespace_footnote_coupling(kind, token, has_following_colon, coupled);
            }
            Event::FootnoteReferenceCoupling {
                kind,
                token,
                follows_space_before_colon,
                coupled,
            } if tracing::enabled!(tracing::Level::DEBUG) => {
                log_footnote_reference_coupling(kind, token, follows_space_before_colon, coupled);
            }
            Event::FragmentClassified { token, kind }
                if tracing::enabled!(tracing::Level::DEBUG) =>
            {
                debug!(
                    token_length = token.chars().count(),
                    kind = ?kind, "fragment classified"
                );
            }
            _ => {}
        }
    }
}
