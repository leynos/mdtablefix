//! Pending-prefix continuation handling for soft-wrapped paragraphs.
//!
//! When a prefixed line (bullet, ordered list, blockquote, footnote
//! definition) opens an inline code span that the source soft-wraps across
//! subsequent lines, [`super::wrap_text`] defers emission into a
//! [`PendingPrefix`] buffer. This module owns the join/update/dispatch
//! state machine that reconciles each continuation chunk with the buffer:
//! it joins the chunks, tracks open-fence state across them, splits spans
//! that close-then-reopen within one chunk, and flushes the buffered
//! paragraph atomically once the span is fully resolved.

use std::borrow::Cow;

use tracing::trace;

use super::{
    paragraph::{
        ContinuationMode,
        ParagraphState,
        ParagraphWriter,
        PendingPrefix,
        PrefixLine,
        TailReflow,
        pending_prefix_for_next_segment,
    },
    tokenize,
    tokenize::{parse_open_code_span, scan_continuation_span_state},
};

mod spacing;
mod span_state;

// The join spacing and span-state helpers are exercised through this module's
// tests, which reach them by `use super::*`; re-exporting keeps that path while
// the implementations live in siblings.
use spacing::{
    join_pending_continuation,
    leading_run_needs_space,
    should_emit_verbatim_for_width,
    suppresses_join_space_after_nested_open_paren,
};
use span_state::{SpanStateUpdate, update_span_state};

#[cfg(test)]
#[path = "continuation_tests.rs"]
mod tests;

/// Joins `text` onto the active pending-prefix buffer and reacts to the
/// resulting span-state update.
///
/// Centralises the join/update/dispatch path shared by the prefixed and
/// non-prefixed continuation branches in `handle_pending_continuation`.
/// Returns silently when `state.pending_prefix` is `None`.
pub(super) fn apply_continuation_chunk(
    text: &str,
    source_line: &str,
    hard_break: bool,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
) {
    if should_emit_verbatim_for_width(text, state)
        && emit_verbatim_for_width(source_line, hard_break, writer, state)
    {
        return;
    }

    let Some(pending) = state.pending_prefix.as_mut() else {
        return;
    };
    pending.original_lines.push(source_line.to_owned());
    let Some(continuation_offset) = join_chunk_with_spacing(pending, text, hard_break) else {
        return;
    };
    apply_span_update(text, continuation_offset, hard_break, writer, state);
}

/// Joins `text` onto the pending buffer and records the space it inserted.
///
/// Returns the byte offset at which `text` begins inside the joined buffer, or
/// `None` when the chunk was empty and nothing was joined. That offset is
/// computed before the join because the span scan reports positions within
/// `text`, which the caller needs in buffer coordinates.
fn join_chunk_with_spacing(
    pending: &mut PendingPrefix,
    text: &str,
    hard_break: bool,
) -> Option<usize> {
    let open_fence_len = pending.open_fence_len.unwrap_or(0);
    let should_join_verbatim = pending.continuation_mode == ContinuationMode::TightCodeSpan;
    // Compute the leading backtick run once; both the offset calculation and
    // `join_pending_continuation` need it, and a fresh scan from each would
    // double-walk `text`.
    let leading_run_len = tokenize::opening_fence_run_len(text.as_bytes(), text);
    let needs_space = !should_join_verbatim
        && leading_run_needs_space(leading_run_len, open_fence_len, should_join_verbatim)
        && !suppresses_join_space_after_nested_open_paren(&pending.rest, open_fence_len);
    let inserted_space_offset = if pending.rest.is_empty() || !needs_space {
        None
    } else {
        Some(pending.rest.len())
    };
    let continuation_offset = {
        let pending_len = pending.rest.len();
        pending_len + usize::from(pending_len > 0 && needs_space)
    };
    let joined = join_pending_continuation(&mut pending.rest, text, needs_space);
    if joined && let Some(offset) = inserted_space_offset {
        pending.synthetic_join_spaces.push(offset);
    }
    // The source broke the line whether or not there was text to append, so the
    // marker is recorded before the early return.
    if hard_break {
        pending.hard_break = true;
    }
    joined.then_some(continuation_offset)
}

/// Emits a width-triggered verbatim continuation, reporting whether it did.
///
/// Two buffered shapes take this path: a paragraph already marked for verbatim
/// flushing, which keeps its buffer and flushes immediately, and any other
/// pending prefix, which is taken out and emitted with the source line. Returns
/// `false` when there is no pending prefix, so the caller falls through to the
/// ordinary join.
fn emit_verbatim_for_width(
    source_line: &str,
    hard_break: bool,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
) -> bool {
    if let Some(pending) = state.pending_prefix.as_mut()
        && pending.continuation_mode == ContinuationMode::VerbatimFlush
    {
        trace!(
            ?pending.continuation_mode,
            "flushing original lines for width-triggered verbatim continuation"
        );
        pending.original_lines.push(source_line.to_owned());
        writer.flush_paragraph(state);
        return true;
    }

    let Some(mut pending) = state.pending_prefix.take() else {
        return false;
    };
    pending.original_lines.push(source_line.to_owned());
    writer.emit_pending_with_verbatim_continuation(pending, source_line, hard_break);
    true
}

/// Reacts to what a joined chunk did to the deferred span.
///
/// A still-open span disables tail reflow only in verbatim mode, a reopened span
/// always disables it and may flush through the split, and a resolved span
/// flushes when the source broke the line or reflow was already disallowed.
fn apply_span_update(
    text: &str,
    continuation_offset: usize,
    hard_break: bool,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
) {
    let Some(pending) = state.pending_prefix.as_mut() else {
        return;
    };
    match update_span_state(text, continuation_offset, pending) {
        SpanStateUpdate::StillOpen => disable_reflow_if_verbatim(pending),
        SpanStateUpdate::ClosedAndReopened { split_at, new_len } => {
            disable_tail_reflow_after_reopen(pending);
            if reopen_pending_span(writer, pending, split_at, new_len) {
                writer.flush_paragraph(state);
            }
        }
        SpanStateUpdate::Flush => {
            if hard_break || pending.tail_reflow == TailReflow::Disallowed {
                writer.flush_paragraph(state);
            }
        }
    }
}

/// Disables tail reflow for a still-open span in verbatim mode.
///
/// A span that stays open keeps deferring, which is only safe while the buffer
/// is joined the way the source wrote it. Any other mode leaves reflow enabled,
/// because the span is still there to close later.
fn disable_reflow_if_verbatim(pending: &mut PendingPrefix) {
    if pending.continuation_mode == ContinuationMode::VerbatimFlush {
        pending.tail_reflow = TailReflow::Disallowed;
        trace!(
            mode = ?pending.continuation_mode,
            boundary = "span_still_open",
            line_count = pending.original_lines.len(),
            "disabled pending-prefix tail reflow"
        );
    }
}

/// Disables tail reflow for a span that closed and reopened within one chunk.
///
/// The two spans are separate source units, so the buffer must be emitted
/// verbatim rather than reflowed across the boundary.
fn disable_tail_reflow_after_reopen(pending: &mut PendingPrefix) {
    pending.tail_reflow = TailReflow::Disallowed;
    trace!(
        mode = ?pending.continuation_mode,
        boundary = "span_reopened",
        line_count = pending.original_lines.len(),
        "disabled pending-prefix tail reflow after a reopened span"
    );
}

/// Split a close-and-reopen chunk into an emitted span and a newly pending one.
///
/// Synthetic join offsets and fence state are rebased to the new tail so later
/// wrapping cannot join text across two logically separate code spans.
fn reopen_pending_span(
    writer: &mut ParagraphWriter<'_>,
    pending: &mut PendingPrefix,
    split_at: usize,
    new_len: usize,
) -> bool {
    emit_pending_prefix_segment(writer, pending, split_at);
    let pending_rest = format!(
        "{ticks}{tail}",
        ticks = "`".repeat(new_len),
        tail = &pending.rest[split_at + new_len..],
    );
    let opener_at_eol = pending_rest[new_len..].trim().is_empty();
    let pending_rest = if opener_at_eol {
        "`".repeat(new_len)
    } else {
        pending_rest
    };
    pending.rest = pending_rest;
    pending.synthetic_join_spaces = pending
        .synthetic_join_spaces
        .iter()
        .filter_map(|offset| offset.checked_sub(split_at))
        .filter(|offset| *offset < pending.rest.len())
        .collect();
    pending.open_fence_len = scan_continuation_span_state(pending.rest.as_str(), 0)
        .filter(|len| *len > 0)
        .or_else(|| parse_open_code_span(pending.rest.as_str()).map(|(len, _)| len));
    let continuation_mode = if opener_at_eol {
        ContinuationMode::TightCodeSpan
    } else {
        ContinuationMode::Normalize
    };
    trace!(
        ?continuation_mode,
        opener_at_eol, new_len, "selected continuation mode after close/reopen split"
    );
    pending.continuation_mode = continuation_mode;
    pending.hard_break = false;
    pending.open_fence_len.is_none()
}

/// Emit the closed prefix segment and advance the continuation indentation.
///
/// A split span must retain the original prefix once, then use its continuation
/// prefix for the reopened tail to keep list and blockquote geometry stable.
fn emit_pending_prefix_segment(
    writer: &mut ParagraphWriter<'_>,
    pending: &mut PendingPrefix,
    split_at: usize,
) {
    if split_at == 0 {
        return;
    }

    let flushed = pending.rest[..split_at].to_string();
    if flushed.is_empty() {
        return;
    }

    // Advances `used_prefix` so later split segments use continuation indent.
    let prefix = pending_prefix_for_next_segment(pending);
    let prefix_line = PrefixLine {
        prefix: Cow::Owned(prefix),
        rest: flushed.as_str(),
        repeat_prefix: pending.repeat_prefix,
        outer_prefix: pending.outer_prefix.as_deref().map(Cow::Borrowed),
    };
    writer.append_wrapped_with_prefix_width(&prefix_line, pending.rest_width);
    if pending.hard_break {
        writer.ensure_trailing_hard_break_on_last_line();
    }
}
