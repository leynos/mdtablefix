//! `tracing` integration for inline-classification domain events.

use tracing::{debug, trace};

use super::observer::{Event, Observer};

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
            Event::DateSequenceMatched { start, end }
                if tracing::enabled!(tracing::Level::DEBUG) =>
            {
                debug!(start, end, "matched date sequence");
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
