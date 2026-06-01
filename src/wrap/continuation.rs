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

use super::{
    paragraph::{ContinuationMode, ParagraphState, ParagraphWriter, PendingPrefix, PrefixLine},
    tokenize,
    tokenize::{parse_open_code_span, position_after_close, scan_continuation_span_state},
};

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
    let Some(pending) = state.pending_prefix.as_mut() else {
        return;
    };
    pending.original_lines.push(source_line.to_string());

    let open_fence_len = pending.open_fence_len.unwrap_or(0);
    let should_join_verbatim = pending.continuation_mode == ContinuationMode::TightCodeSpan;
    // Compute the leading backtick run once; both the offset calculation and
    // `join_pending_continuation` need it, and a fresh scan from each would
    // double-walk `text`.
    let leading_run_len = tokenize::opening_fence_run_len(text.as_bytes(), text);
    let needs_space = !should_join_verbatim
        && leading_run_needs_space(
            leading_run_len,
            open_fence_len,
            pending.continuation_mode == ContinuationMode::TightCodeSpan,
        );
    let continuation_offset = {
        let pending_len = pending.rest.len();
        pending_len + usize::from(pending_len > 0 && needs_space)
    };
    let joined = join_pending_continuation(
        &mut pending.rest,
        text,
        open_fence_len,
        leading_run_len,
        should_join_verbatim,
        pending.continuation_mode == ContinuationMode::TightCodeSpan,
    );
    if hard_break {
        pending.hard_break = true;
    }
    if !joined {
        return;
    }
    match update_span_state(text, continuation_offset, pending) {
        SpanStateUpdate::StillOpen => {}
        SpanStateUpdate::ClosedAndReopened { split_at, new_len } => {
            emit_pending_prefix_segment(writer, pending, split_at);
            let pending_rest = format!(
                "{ticks}{tail}",
                ticks = "`".repeat(new_len),
                tail = &pending.rest[split_at + new_len..],
            );
            let opener_at_eol = pending_rest["`".repeat(new_len).len()..].trim().is_empty();
            let pending_rest = if opener_at_eol {
                "`".repeat(new_len)
            } else {
                pending_rest
            };
            pending.rest = pending_rest;
            pending.open_fence_len = Some(new_len);
            pending.continuation_mode = if opener_at_eol {
                ContinuationMode::TightCodeSpan
            } else {
                ContinuationMode::Normalize
            };
            pending.hard_break = false;
        }
        SpanStateUpdate::Flush => {
            writer.flush_paragraph(state);
        }
    }
}

/// Join a soft-wrapped continuation onto pending prefixed text.
///
/// Never synthesizes closing fences. Only backticks present in the source may
/// close an open span; when the continuation does not begin with the matching
/// closing fence run, a single space is inserted before appending. The caller
/// is responsible for measuring the leading backtick run with
/// `tokenize::opening_fence_run_len` and threading it in via
/// `leading_run_len` so we avoid double-walking `continuation`.
fn join_pending_continuation(
    existing: &mut String,
    continuation: &str,
    fence_len: usize,
    leading_run_len: Option<usize>,
    verbatim_continuations: bool,
    opener_at_eol: bool,
) -> bool {
    if continuation.is_empty() {
        return false;
    }

    let needs_space = !verbatim_continuations
        && leading_run_needs_space(leading_run_len, fence_len, opener_at_eol);
    if !existing.is_empty() && needs_space {
        existing.push(' ');
    }
    existing.push_str(continuation);

    true
}

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
        position_after_close(continuation, existing_fence)?
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

/// Decide whether a continuation chunk needs a leading space when joined
/// onto the pending buffer.
///
/// A space is required unless the chunk starts with a backtick run whose
/// length matches the currently open fence (i.e. the chunk begins with the
/// span's closing fence). `leading_run_len` is the precomputed length of any
/// such opening backtick run, as returned by `opening_fence_run_len`.
fn leading_run_needs_space(
    leading_run_len: Option<usize>,
    open_fence_len: usize,
    opener_at_eol: bool,
) -> bool {
    match leading_run_len {
        Some(run_len) => run_len != open_fence_len,
        // CommonMark turns a source line ending into a space inside code
        // spans. For an opener at EOL, preserving that space creates MD038
        // failures, so keep the continuation tight instead.
        None if open_fence_len > 0 && opener_at_eol => false,
        None => true,
    }
}

fn emit_pending_prefix_segment(
    writer: &mut ParagraphWriter<'_>,
    pending: &PendingPrefix,
    split_at: usize,
) {
    if split_at == 0 {
        return;
    }

    let flushed = &pending.rest[..split_at];
    if flushed.is_empty() {
        return;
    }

    let prefix_line = PrefixLine {
        prefix: Cow::Borrowed(pending.prefix.as_str()),
        rest: flushed,
        repeat_prefix: pending.repeat_prefix,
    };
    writer.append_wrapped_with_prefix_width(&prefix_line, pending.rest_width);
    if pending.hard_break {
        writer.ensure_trailing_hard_break_on_last_line();
    }
}

/// Outcome of inspecting a continuation chunk against the current pending span.
///
/// When `scan_continuation_span_state` reports no active span, the caller
/// falls back to detecting whether a new opener begins in the latest
/// continuation chunk so a newly opened span can continue deferral.
enum SpanStateUpdate {
    /// The span remains open and should keep deferring.
    StillOpen,
    /// The existing span closed and a new one opened at `split_at`.
    ClosedAndReopened { split_at: usize, new_len: usize },
    /// The buffered paragraph should be flushed now.
    Flush,
}

fn update_span_state(
    continuation: &str,
    continuation_offset: usize,
    pending: &mut PendingPrefix,
) -> SpanStateUpdate {
    let raw_fence = pending.open_fence_len.unwrap_or(0);
    match scan_continuation_span_state(continuation, raw_fence) {
        Some(n) if n > 0 => {
            if split_reopen_span(continuation, continuation_offset, raw_fence).is_some() {
                pending.continuation_mode = ContinuationMode::VerbatimFlush;
            }
            pending.open_fence_len = Some(n);
            SpanStateUpdate::StillOpen
        }
        _ => {
            pending.open_fence_len = None;
            if let Some((split_at, new_len)) =
                split_reopen_span(continuation, continuation_offset, raw_fence)
            {
                return SpanStateUpdate::ClosedAndReopened { split_at, new_len };
            }
            pending.continuation_mode =
                if raw_fence > 0 && closing_fence_tail_starts_word(continuation, raw_fence) {
                    ContinuationMode::VerbatimFlush
                } else {
                    ContinuationMode::Normalize
                };
            SpanStateUpdate::Flush
        }
    }
}

fn closing_fence_tail_starts_word(continuation: &str, raw_fence: usize) -> bool {
    let Some(close_end) = position_after_close(continuation, raw_fence) else {
        return false;
    };
    continuation[close_end..]
        .chars()
        .next()
        .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
}
