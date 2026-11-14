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
use unicode_width::UnicodeWidthStr;

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
    tokenize::{parse_open_code_span, position_after_close, scan_continuation_span_state},
};

mod span_state;

// The span-state transition is exercised through this module's tests, which
// reach it by `use super::*`; re-exporting here keeps that path while the
// implementation lives in the sibling.
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
    if should_emit_verbatim_for_width(text, state) {
        if let Some(pending) = state.pending_prefix.as_mut()
            && pending.continuation_mode == ContinuationMode::VerbatimFlush
        {
            trace!(
                ?pending.continuation_mode,
                "flushing original lines for width-triggered verbatim continuation"
            );
            pending.original_lines.push(source_line.to_owned());
            writer.flush_paragraph(state);
            return;
        }

        if let Some(mut pending) = state.pending_prefix.take() {
            pending.original_lines.push(source_line.to_owned());
            writer.emit_pending_with_verbatim_continuation(pending, source_line, hard_break);
            return;
        }
    }

    let Some(pending) = state.pending_prefix.as_mut() else {
        return;
    };
    pending.original_lines.push(source_line.to_owned());

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
    if hard_break {
        pending.hard_break = true;
    }
    if !joined {
        return;
    }
    match update_span_state(text, continuation_offset, pending) {
        SpanStateUpdate::StillOpen => {
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
        SpanStateUpdate::ClosedAndReopened { split_at, new_len } => {
            pending.tail_reflow = TailReflow::Disallowed;
            trace!(
                mode = ?pending.continuation_mode,
                boundary = "span_reopened",
                offset = split_at,
                line_count = pending.original_lines.len(),
                "disabled pending-prefix tail reflow after a reopened span"
            );
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

/// Decide whether wrapping a continuation would exceed its protected width.
///
/// A verbatim flush preserves source text when an open code span cannot be
/// safely reflowed without changing its displayed content or prefix alignment.
fn should_emit_verbatim_for_width(text: &str, state: &ParagraphState) -> bool {
    if text.is_empty() {
        trace!(
            emit = false,
            reason = "empty_text",
            "checked width-triggered verbatim continuation"
        );
        return false;
    }

    let Some(pending) = state.pending_prefix.as_ref() else {
        trace!(
            emit = false,
            reason = "no_pending_prefix",
            "checked width-triggered verbatim continuation"
        );
        return false;
    };

    let open_fence_len = pending.open_fence_len.unwrap_or(0);
    let is_tight_code_span = pending.continuation_mode == ContinuationMode::TightCodeSpan;
    let leading_run_len = tokenize::opening_fence_run_len(text.as_bytes(), text);
    let needs_space = !is_tight_code_span
        && leading_run_needs_space(leading_run_len, open_fence_len, false)
        && !suppresses_join_space_after_nested_open_paren(&pending.rest, open_fence_len);
    let projected_width = UnicodeWidthStr::width(pending.rest.as_str())
        + usize::from(!pending.rest.is_empty() && needs_space)
        + UnicodeWidthStr::width(text);

    let closes_pending_span_at_end = position_after_close(text, 0, open_fence_len)
        .is_some_and(|close_end| text[close_end..].trim().is_empty())
        && matches!(
            scan_continuation_span_state(text, open_fence_len),
            None | Some(0)
        );

    let emit = closes_pending_span_at_end && projected_width > pending.rest_width;
    trace!(
        emit,
        ?pending.continuation_mode,
        projected_width,
        rest_width = pending.rest_width,
        closes_pending_span_at_end,
        open_fence_len,
        "checked width-triggered verbatim continuation"
    );
    emit
}

/// Join a soft-wrapped continuation onto pending prefixed text.
///
/// Never synthesizes closing fences. Only backticks present in the source may
/// close an open span; when the continuation does not begin with the matching
/// closing fence run, the caller passes `needs_space` so a single space is
/// inserted before appending.
fn join_pending_continuation(existing: &mut String, continuation: &str, needs_space: bool) -> bool {
    if continuation.is_empty() {
        return false;
    }

    if !existing.is_empty() && needs_space {
        existing.push(' ');
    }
    existing.push_str(continuation);

    true
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

/// Preserve a tight join after nested parentheses inside an open code span.
///
/// The second unmatched opener proves that adding Markdown's usual join space
/// would become payload rather than a separator between prose tokens.
fn suppresses_join_space_after_nested_open_paren(existing: &str, open_fence_len: usize) -> bool {
    if open_fence_len == 0 || !existing.ends_with('(') {
        return false;
    }

    let Some((_, code_tail)) = parse_open_code_span(existing) else {
        return false;
    };

    unclosed_parenthesis_depth(code_tail) > 1
}

/// Count unmatched opening parentheses without underflowing on extra closers.
///
/// Only nesting depth matters to continuation spacing, so a saturating scan
/// intentionally treats a leading closer as already balanced.
fn unclosed_parenthesis_depth(text: &str) -> usize {
    text.chars().fold(0usize, |depth, ch| match ch {
        '(' => depth.saturating_add(1),
        ')' => depth.saturating_sub(1),
        _ => depth,
    })
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
