//! `tracing` integration for inline-classification domain events.

use tracing::{debug, trace};

use super::observer::{Event, Observer};

/// Translates domain events into the crate's existing `tracing` events.
///
/// The adapter owns every vendor-specific concern: the `tracing` level gate and
/// any derived value that costs more than a copy (Unicode length counts and
/// bounded snippets). Domain emitters hand over only borrowed slices, so a
/// disabled subscriber pays nothing beyond the branch below.
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
                let (snippet, truncated) = trace_text_snippet(token);
                debug!(token = %snippet, truncated, kind = ?kind, "fragment classified");
            }
            _ => {}
        }
    }
}

/// Returns a UTF-8-safe prefix of `text` for debug logging.
///
/// The prefix contains at most 80 bytes and never splits a multi-byte
/// character. The second tuple element is `true` when `text` was shortened.
/// This lives in the adapter so the truncation scan only runs once the level
/// gate above has confirmed the event will be emitted.
fn trace_text_snippet(text: &str) -> (&str, bool) {
    const MAX_TRACE_BYTES: usize = 80;
    if text.len() <= MAX_TRACE_BYTES {
        return (text, false);
    }

    let mut byte_end = 0;
    for (idx, ch) in text.char_indices() {
        let next_end = idx + ch.len_utf8();
        if next_end > MAX_TRACE_BYTES {
            break;
        }
        byte_end = next_end;
    }

    (&text[..byte_end], true)
}

#[cfg(test)]
mod trace_snippet_tests {
    //! Tests for the `trace_text_snippet` helper.
    //!
    //! Verifies that the UTF-8-safe truncation helper produces a slice at a
    //! valid character boundary and sets the truncation flag correctly.

    use super::trace_text_snippet;

    #[test]
    fn trace_text_snippet_truncates_on_char_boundary() {
        let ascii = "a".repeat(79);
        let text = format!("{ascii}étail");
        let (snippet, truncated) = trace_text_snippet(&text);

        assert!(truncated);
        assert_eq!(snippet, ascii.as_str());
        assert!(snippet.is_char_boundary(snippet.len()));
    }
}

#[cfg(test)]
mod trace_snippet_proptests {
    //! Property tests for `trace_text_snippet` invariants.
    //!
    //! Verifies on arbitrary Unicode input that the helper never panics, the
    //! result is a valid UTF-8 slice of at most 80 bytes, and the truncation
    //! flag accurately reflects whether the input exceeded that limit.

    use proptest::prelude::*;

    use super::trace_text_snippet;

    proptest! {
        #[test]
        fn trace_text_snippet_never_panics(s in "\\PC*") {
            let (snippet, truncated) = trace_text_snippet(&s);
            // Invariant 1: result is always valid UTF-8 at a char boundary.
            assert!(snippet.is_char_boundary(snippet.len()));
            // Invariant 2: result never exceeds 80 bytes.
            assert!(snippet.len() <= 80);
            // Invariant 3: truncation flag is accurate.
            assert_eq!(truncated, s.len() > 80);
        }
    }
}
