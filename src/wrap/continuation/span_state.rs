//! Deferred code-span state for pending-prefix continuations.
//!
//! A prefixed line whose inline code span continues onto later source lines is
//! buffered until the span resolves. These helpers decide what one continuation
//! chunk did to that deferral: whether the span is still open, whether it closed
//! and reopened within the chunk, or whether the buffer can be flushed. They are
//! separate from the join and dispatch path in [`super`] so the state transition
//! can be read, and tested, without the writing side beside it.
//!
//! Nothing here emits output; each function reports a [`SpanStateUpdate`] and
//! leaves the emission decision to the caller.

use tracing::trace;

use super::super::{
    paragraph::{ContinuationMode, PendingPrefix},
    tokenize::{parse_open_code_span, position_after_close, scan_continuation_span_state},
};

/// Locates the byte offset of a newly opened code span inside `continuation`.
///
/// When `existing_fence` is non-zero, the helper first advances past the
/// matching close of the pre-existing span before searching for the new
/// opener. This prevents `parse_open_code_span` from pairing the closer of
/// the existing span with the opener of the new span, which would otherwise
/// hide the close/reopen boundary.
///
/// Returns the absolute byte offset of the new opener (relative to the
/// surrounding pending buffer, derived from `continuation_offset`) and the
/// fence length of the new opener.
fn split_reopen_span(
    continuation: &str,
    continuation_offset: usize,
    existing_fence: usize,
) -> Option<(usize, usize)> {
    let search_offset = if existing_fence > 0 {
        position_after_close(continuation, 0, existing_fence)?
    } else {
        0
    };

    let remainder = &continuation[search_offset..];
    let (open_len, open_tail) = parse_open_code_span(remainder)?;
    if open_tail.is_empty() {
        return None;
    }

    let open_run_end = remainder.len().checked_sub(open_tail.len() + open_len)?;
    let in_continuation = search_offset.checked_add(open_run_end)?;
    continuation_offset
        .checked_add(in_continuation)
        .map(|split_at| (split_at, open_len))
}

/// Outcome of inspecting a continuation chunk against the current pending span.
///
/// When `scan_continuation_span_state` reports no active span, the caller
/// falls back to detecting whether a new opener begins in the latest
/// continuation chunk so a newly opened span can continue deferral.
pub(super) enum SpanStateUpdate {
    /// The span remains open and should keep deferring.
    StillOpen,
    /// The existing span closed and a new one opened at `split_at`.
    ClosedAndReopened {
        /// Byte boundary where the completed span leaves the pending buffer.
        split_at: usize,
        /// Backtick-run length that opens the replacement pending span.
        new_len: usize,
    },
    /// The buffered paragraph should be flushed now.
    Flush,
}

/// Update deferred span state after appending one continuation chunk.
///
/// The result distinguishes an unfinished span, a close-and-reopen boundary,
/// and a safe flush so callers never merge text across a fence transition.
pub(super) fn update_span_state(
    continuation: &str,
    continuation_offset: usize,
    pending: &mut PendingPrefix,
) -> SpanStateUpdate {
    let raw_fence = pending.open_fence_len.unwrap_or(0);
    match scan_continuation_span_state(continuation, raw_fence) {
        Some(new_fence) if new_fence > 0 => {
            record_open_span(
                continuation,
                continuation_offset,
                raw_fence,
                new_fence,
                pending,
            );
            SpanStateUpdate::StillOpen
        }
        _ => resolve_closed_span(continuation, continuation_offset, raw_fence, pending),
    }
}

/// Records that the span is still open, noting a close-then-reopen chunk.
///
/// A chunk that closes the buffered span and opens another in the same chunk
/// must be emitted verbatim: the two spans are separate source units, and
/// reflowing across the boundary would join text that the source kept apart.
fn record_open_span(
    continuation: &str,
    continuation_offset: usize,
    raw_fence: usize,
    new_fence: usize,
    pending: &mut PendingPrefix,
) {
    if split_reopen_span(continuation, continuation_offset, raw_fence).is_some() {
        trace!(
            continuation_mode = ?ContinuationMode::VerbatimFlush,
            raw_fence,
            new_fence_len = new_fence,
            "selected verbatim continuation mode for split reopen"
        );
        pending.continuation_mode = ContinuationMode::VerbatimFlush;
    }
    pending.open_fence_len = Some(new_fence);
}

/// Resolves the buffer once the chunk leaves no open span behind.
///
/// A chunk that reopens a span is split at that boundary; otherwise the buffer
/// is ready to flush, in verbatim mode when payload follows the closer and in
/// normalize mode otherwise.
fn resolve_closed_span(
    continuation: &str,
    continuation_offset: usize,
    raw_fence: usize,
    pending: &mut PendingPrefix,
) -> SpanStateUpdate {
    pending.open_fence_len = None;
    if let Some((split_at, new_len)) =
        split_reopen_span(continuation, continuation_offset, raw_fence)
    {
        return SpanStateUpdate::ClosedAndReopened { split_at, new_len };
    }
    let continuation_mode = flush_mode_for(continuation, raw_fence);
    trace!(
        ?continuation_mode,
        raw_fence, "selected continuation mode before flushing pending prefix"
    );
    pending.continuation_mode = continuation_mode;
    SpanStateUpdate::Flush
}

/// Selects the flush mode for a chunk that leaves no span open.
///
/// Reflowing a word joined directly to the closer could introduce a space and
/// change code-span payload, so that boundary selects the verbatim path. The
/// guard keeps a chunk with no pending span in normalize mode, where the tail
/// test has nothing to compare against.
fn flush_mode_for(continuation: &str, raw_fence: usize) -> ContinuationMode {
    if raw_fence > 0 && closing_fence_tail_starts_word(continuation, raw_fence) {
        ContinuationMode::VerbatimFlush
    } else {
        ContinuationMode::Normalize
    }
}

/// Detect payload immediately after a closing fence that requires verbatim use.
///
/// Reflowing a word joined directly to the closer could introduce a space and
/// change code-span payload, so that boundary selects the verbatim path.
fn closing_fence_tail_starts_word(continuation: &str, raw_fence: usize) -> bool {
    let Some(close_end) = position_after_close(continuation, 0, raw_fence) else {
        return false;
    };
    continuation[close_end..]
        .chars()
        .next()
        .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
}
