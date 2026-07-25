//! `tracing` integration for inline-classification domain events.

use tracing::{debug, trace};

use super::observer::{Event, Observer};

/// Translates domain events into the crate's existing `tracing` events.
pub(crate) struct TracingObserver;

impl Observer for TracingObserver {
    fn observe(&mut self, event: Event<'_>) {
        match event {
            Event::FootnoteReferenceParsed { token_length }
                if tracing::enabled!(tracing::Level::DEBUG) =>
            {
                debug!(token_length, "footnote reference parsed");
            }
            Event::LinkOrImageParsed {
                token_length,
                is_image,
            } if tracing::enabled!(tracing::Level::DEBUG) => {
                debug!(token_length, is_image, "link or image parsed");
            }
            Event::FootnoteEndNotFound { start, reason }
                if tracing::enabled!(tracing::Level::TRACE) =>
            {
                trace!(start, reason, "footnote end not found");
            }
            Event::FootnoteLabelRecognized {
                start,
                end,
                token_length,
            } if tracing::enabled!(tracing::Level::TRACE) => {
                trace!(start, end, token_length, "footnote label span recognized");
            }
            Event::FootnoteRefChecked { token, result }
                if tracing::enabled!(tracing::Level::TRACE) =>
            {
                trace!(
                    token_length = token.chars().count(),
                    result, "footnote reference checked"
                );
            }
            Event::FragmentClassified {
                token,
                truncated,
                kind,
            } if tracing::enabled!(tracing::Level::DEBUG) => {
                debug!(token = %token, truncated, kind = ?kind, "fragment classified");
            }
            _ => {}
        }
    }
}
