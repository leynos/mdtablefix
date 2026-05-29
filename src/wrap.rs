//! Utilities for wrapping Markdown lines.
//!
//! These helpers reflow paragraphs and list items while preserving inline code
//! spans, fenced code blocks, and other prefixes. Width calculations rely on
//! `UnicodeWidthStr::width` from the `unicode-width` crate as described in
//! `docs/architecture.md#unicode-width-handling`.
//!
//! The [`Token`] enum and [`tokenize_markdown`] function are public so callers
//! can perform custom token-based processing.

use std::borrow::Cow;

mod block;
mod fence;
mod inline;
mod link_reference;
mod paragraph;
mod tokenize;
use block::{BLOCKQUOTE_RE, BULLET_RE, FOOTNOTE_RE};
pub(crate) use block::{BlockKind, classify_block};
/// Fence-detection utilities re-exported for downstream callers.
///
/// [`FenceTracker`] maintains fenced code-block state across lines, which is
/// useful for callers that process Markdown incrementally. [`is_fence`]
/// inspects one line and returns the fence components (indentation, marker,
/// info string) when the line opens a fenced code block, or `None` otherwise.
pub use fence::{FenceTracker, is_fence};
pub(crate) use link_reference::LinkReferenceMatcher;
use paragraph::{ParagraphState, ParagraphWriter, PendingPrefix, PrefixLine};
/// Token emitted by the `tokenize::segment_inline` parser and used by
/// higher-level wrappers.
///
/// Downstream callers inspect [`Token<'a>`] when implementing bespoke
/// wrapping logic. The `'a` lifetime parameter ties each token to the source
/// text, avoiding unnecessary allocation.
///
/// Re-export these so callers of [`crate::textproc`] can implement custom
/// transformations without depending on internal modules.
pub use tokenize::Token;
#[doc(inline)]
pub use tokenize::tokenize_markdown;
// Re-exported for unit tests; not used in production code.
#[cfg(test)]
pub(crate) use tokenize::{continuation_begins_with_closing_fence, has_unclosed_code_span};
use tokenize::{parse_open_code_span, position_after_close, scan_continuation_span_state};

// Permit GFM task list markers with flexible spacing and missing post-marker
// spaces in Markdown.

fn is_indented_code_line(line: &str) -> bool {
    // CommonMark expands tabs to four spaces when measuring indentation.
    let indent_width = line
        .as_bytes()
        .iter()
        .take_while(|b| **b == b' ' || **b == 0x09)
        .map(|&b| if b == 0x09 { 4 } else { 1 })
        .sum::<usize>();

    indent_width >= 4 && line.chars().any(|c| !c.is_whitespace())
}

fn is_table_or_separator(line: &str) -> bool {
    line.trim_start().starts_with('|') || crate::table::SEP_RE.is_match(line.trim())
}

fn is_passthrough_block(block_kind: Option<BlockKind>, line: &str) -> bool {
    is_table_or_separator(line)
        || matches!(
            block_kind,
            Some(
                BlockKind::Heading
                    | BlockKind::MarkdownlintDirective
                    | BlockKind::LinkReferenceDefinition,
            )
        )
        || line.trim().is_empty()
        || is_indented_code_line(line)
}

fn prefix_line(line: &str) -> Option<PrefixLine<'_>> {
    if let Some(cap) = BULLET_RE.captures(line) {
        let prefix = cap.get(1).expect("bullet regex capture").as_str();
        let rest = cap.get(2).expect("bullet regex remainder capture").as_str();
        return Some(PrefixLine {
            prefix: Cow::Borrowed(prefix),
            rest,
            repeat_prefix: false,
        });
    }

    if let Some(cap) = FOOTNOTE_RE.captures(line) {
        let prefix = cap.get(1).expect("footnote prefix capture").as_str();
        let marker = cap.get(2).expect("footnote marker capture").as_str();
        let rest = cap
            .get(3)
            .expect("footnote regex remainder capture")
            .as_str();
        return Some(PrefixLine {
            prefix: Cow::Owned(format!("{prefix}{marker}")),
            rest,
            repeat_prefix: false,
        });
    }

    BLOCKQUOTE_RE.captures(line).map(|cap| PrefixLine {
        prefix: Cow::Borrowed(cap.get(1).expect("blockquote prefix capture").as_str()),
        rest: cap
            .get(2)
            .expect("blockquote regex remainder capture")
            .as_str(),
        repeat_prefix: true,
    })
}

fn line_break_parts(line: &str) -> (String, bool) {
    let trimmed_end = line.trim_end();
    let text_without_html_breaks = trimmed_end
        .trim_end_matches("<br>")
        .trim_end_matches("<br/>")
        .trim_end_matches("<br />");

    let is_trailing_spaces = line.ends_with("  ");
    let is_html_br = trimmed_end != text_without_html_breaks;
    let backslash_count = trimmed_end.chars().rev().take_while(|&c| c == '\\').count();
    let is_backslash_escape = backslash_count % 2 == 1;
    let hard_break = is_trailing_spaces || is_html_br || is_backslash_escape;
    let text = text_without_html_breaks
        .trim_start()
        .trim_end_matches(' ')
        .to_string();
    (text, hard_break)
}

/// Join a soft-wrapped continuation onto pending prefixed text.
///
/// Never synthesises closing fences. Only backticks present in the source may
/// close an open span; when the continuation does not begin with the matching
/// closing fence run, a single space is inserted before appending.
fn join_pending_continuation(existing: &mut String, continuation: &str, fence_len: usize) -> bool {
    if continuation.is_empty() {
        return false;
    }

    let bytes = continuation.as_bytes();
    let needs_space = !{
        if let Some(run_len) = tokenize::opening_fence_run_len(bytes, continuation) {
            run_len == fence_len
        } else {
            false
        }
    };

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

fn continuation_needs_leading_space(continuation: &str, open_fence_len: usize) -> bool {
    let run_len = tokenize::opening_fence_run_len(continuation.as_bytes(), continuation);
    if let Some(run_len) = run_len {
        run_len != open_fence_len
    } else {
        true
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

fn handle_pending_continuation(
    line: &str,
    block_kind: Option<BlockKind>,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
    link_matcher: LinkReferenceMatcher,
    link_title_window: &mut link_reference::LinkTitleWindow,
) {
    if let Some(prefix_line) = prefix_line(line) {
        let matches_pending = state.pending_prefix.as_ref().is_some_and(|pending| {
            prefix_line.repeat_prefix && pending.prefix == prefix_line.prefix.as_ref()
        });
        if matches_pending {
            let (text, hard_break) = line_break_parts(prefix_line.rest);
            apply_continuation_chunk(&text, hard_break, writer, state);
            return;
        }

        writer.handle_prefix_line(state, &prefix_line);
        return;
    }

    if is_passthrough_block(block_kind, line) {
        if matches!(block_kind, Some(BlockKind::LinkReferenceDefinition))
            && link_matcher.standalone_title_need(line) == Some(true)
        {
            link_title_window.observe_bare_definition();
        }
        writer.push_verbatim(state, line);
        return;
    }

    let (text, hard_break) = line_break_parts(line);
    if state.pending_prefix.is_none() {
        return;
    }
    apply_continuation_chunk(&text, hard_break, writer, state);
}

/// Joins `text` onto the active pending-prefix buffer and reacts to the
/// resulting span-state update.
///
/// Centralises the join/update/dispatch path shared by the prefixed and
/// non-prefixed continuation branches in `handle_pending_continuation`.
/// Caller must ensure `state.pending_prefix` is `Some` before invoking.
fn apply_continuation_chunk(
    text: &str,
    hard_break: bool,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
) {
    let Some(pending) = state.pending_prefix.as_mut() else {
        return;
    };

    let open_fence_len = pending.open_fence_len.unwrap_or(0);
    let continuation_offset = {
        let pending_len = pending.rest.len();
        let needs_space = continuation_needs_leading_space(text, open_fence_len);
        pending_len + usize::from(pending_len > 0 && needs_space)
    };
    let joined = join_pending_continuation(&mut pending.rest, text, open_fence_len);
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
            pending.rest = pending_rest;
            pending.open_fence_len = Some(new_len);
            pending.hard_break = false;
        }
        SpanStateUpdate::Flush => {
            writer.flush_paragraph(state);
        }
    }
}

/// Updates the cached open fence length from the incremental span scan result and
/// returns what to do with the deferred prefix buffer.
///
/// When `scan_continuation_span_state` reports no active span, this falls back
/// to detecting whether a new opener begins in the latest continuation chunk so
/// a newly opened span can continue deferral.
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
            // A span is still open at the end of the chunk, but it may be
            // a *different* span: the pre-existing span A closed and a new
            // span B opened within this same continuation. Detect that
            // boundary so the trailing opener does not silently grow the
            // pending buffer across both spans. The closer for span B is
            // not in the source, so synthesise one at the end of the
            // buffer; this keeps span B atomic ("`4.1.1`") and lets
            // subsequent continuations be appended as plain text.
            if let Some((_, new_len)) =
                split_reopen_span(continuation, continuation_offset, raw_fence)
            {
                pending.rest.push_str(&"`".repeat(new_len));
                pending.open_fence_len = None;
                return SpanStateUpdate::StillOpen;
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
            let had_open = raw_fence != 0;
            if !had_open || pending.hard_break {
                SpanStateUpdate::Flush
            } else {
                SpanStateUpdate::StillOpen
            }
        }
    }
}

/// Wrap text lines to the given width.
///
/// # Panics
/// Panics if regex captures fail unexpectedly.
#[must_use]
pub fn wrap_text(lines: &[String], width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut state = ParagraphState::default();
    let mut writer = ParagraphWriter::new(&mut out, width);
    // Track fenced code blocks so wrapping honours shared fence semantics.
    let mut fence_tracker = FenceTracker::default();
    let link_matcher = link_reference::LinkReferenceMatcher::production();
    let mut link_title_window = link_reference::LinkTitleWindow::default();

    for line in lines {
        if fence::handle_fence_line(line, &mut writer, &mut state, &mut fence_tracker) {
            link_title_window.observe_fence_context();
            continue;
        }

        if fence_tracker.in_fence() {
            link_title_window.observe_fence_context();
            writer.push_verbatim(&mut state, line);
            continue;
        }

        if let Some(outcome) = link_title_window.observe_next_line(line, link_matcher)
            && outcome == link_reference::LinkTitleWindowOutcome::EmitVerbatim
        {
            writer.push_verbatim(&mut state, line);
            continue;
        }

        let block_kind = classify_block(line, link_matcher);

        if state.pending_prefix.is_some() {
            handle_pending_continuation(
                line,
                block_kind,
                &mut writer,
                &mut state,
                link_matcher,
                &mut link_title_window,
            );
            continue;
        }

        if is_passthrough_block(block_kind, line) {
            if matches!(block_kind, Some(BlockKind::LinkReferenceDefinition))
                && link_matcher.standalone_title_need(line) == Some(true)
            {
                link_title_window.observe_bare_definition();
            }
            writer.push_verbatim(&mut state, line);
            continue;
        }

        if let Some(prefix_line) = prefix_line(line) {
            writer.handle_prefix_line(&mut state, &prefix_line);
            continue;
        }

        state.note_indent(line);
        let (text, hard_break) = line_break_parts(line);
        state.push(text, hard_break);
    }

    writer.flush_paragraph(&mut state);
    out
}

#[cfg(test)]
mod tests;
