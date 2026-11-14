//! Join spacing and width-triggered verbatim decisions for continuations.
//!
//! Joining a soft-wrapped chunk onto a pending buffer is not simple
//! concatenation. A space may have to be synthesised so two prose words do not
//! fuse, and that space must be suppressed when the chunk begins with the
//! closing fence of the open span or when Markdown's join space would become
//! code payload. Separately, a chunk that closes the open span may prove that
//! reflowing the buffer would exceed its protected width, in which case the
//! source lines are emitted verbatim instead.
//!
//! Both decisions are queries over the pending buffer and the chunk, so they
//! live here rather than in the join-and-dispatch path in [`super`]. Nothing in
//! this module mutates the pending buffer or writes output.

use tracing::trace;
use unicode_width::UnicodeWidthStr;

use super::super::{
    paragraph::{ContinuationMode, ParagraphState},
    tokenize::{self, parse_open_code_span, position_after_close, scan_continuation_span_state},
};

/// Decide whether wrapping a continuation would exceed its protected width.
///
/// A verbatim flush preserves source text when an open code span cannot be
/// safely reflowed without changing its displayed content or prefix alignment.
pub(super) fn should_emit_verbatim_for_width(text: &str, state: &ParagraphState) -> bool {
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
pub(super) fn join_pending_continuation(
    existing: &mut String,
    continuation: &str,
    needs_space: bool,
) -> bool {
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
pub(super) fn leading_run_needs_space(
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
pub(super) fn suppresses_join_space_after_nested_open_paren(
    existing: &str,
    open_fence_len: usize,
) -> bool {
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
