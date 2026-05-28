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
use paragraph::{ParagraphState, ParagraphWriter, PrefixLine};
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
#[doc(hidden)]
pub use tokenize::continuation_begins_with_closing_fence;
#[doc(hidden)]
pub use tokenize::has_unclosed_code_span;
#[doc(inline)]
pub use tokenize::tokenize_markdown;
use tokenize::{parse_open_code_span, scan_continuation_span_state};

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
fn join_pending_continuation(existing: &mut String, continuation: &str, fence_len: usize) {
    if continuation.is_empty() {
        return;
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
        if let Some(pending) = state.pending_prefix.as_mut()
            && prefix_line.repeat_prefix
            && pending.prefix == prefix_line.prefix.as_ref()
        {
            let (text, hard_break) = line_break_parts(prefix_line.rest);
            let fence_before = pending.open_fence_len;
            if !text.is_empty() {
                join_pending_continuation(&mut pending.rest, &text, fence_before.unwrap_or(0));
            }
            if hard_break {
                pending.hard_break = true;
            }
            update_span_state_and_maybe_flush(&text, fence_before, state, writer);
            return;
        }

        writer.handle_prefix_line(state, &prefix_line);
        return;
    }

    if is_table_or_separator(line)
        || matches!(
            block_kind,
            Some(
                BlockKind::Heading
                    | BlockKind::MarkdownlintDirective
                    | BlockKind::LinkReferenceDefinition,
            )
        )
        || line.trim().is_empty()
    {
        if matches!(block_kind, Some(BlockKind::LinkReferenceDefinition))
            && link_matcher.standalone_title_need(line) == Some(true)
        {
            link_title_window.observe_bare_definition();
        }
        writer.push_verbatim(state, line);
        return;
    }

    let (text, hard_break) = line_break_parts(line);
    let Some(pending) = state.pending_prefix.as_mut() else {
        return;
    };

    let fence_before = pending.open_fence_len;
    if !text.is_empty() {
        join_pending_continuation(&mut pending.rest, &text, fence_before.unwrap_or(0));
    }
    if hard_break {
        pending.hard_break = true;
    }
    update_span_state_and_maybe_flush(&text, fence_before, state, writer);
}

/// Updates the cached open fence length from the incremental scan result and
/// decides whether the pending prefix paragraph should be flushed.
///
/// When `scan_continuation_span_state` indicates the prior span closed, this
/// falls back to `has_unclosed_code_span` on the full joined text to detect a
/// new span that may have opened in the same continuation. Flushing is
/// deferred (by the `had_open` guard) when the prior span was open, unless a
/// hard break forces emission.
fn update_span_state_and_maybe_flush(
    continuation: &str,
    fence_before: Option<usize>,
    state: &mut ParagraphState,
    writer: &mut ParagraphWriter<'_>,
) {
    let pending = state
        .pending_prefix
        .as_mut()
        .expect("pending_prefix must be set");
    let raw_fence = fence_before.unwrap_or(0);
    match scan_continuation_span_state(continuation, raw_fence) {
        Some(n) if n > 0 => {
            pending.open_fence_len = Some(n);
        }
        _ => {
            pending.open_fence_len = None;
            if has_unclosed_code_span(pending.rest.as_str()) {
                if let Some((new_len, _)) = parse_open_code_span(&pending.rest) {
                    pending.open_fence_len = Some(new_len);
                }
            } else {
                let had_open = fence_before.is_some();
                if !had_open || pending.hard_break {
                    writer.flush_paragraph(state);
                }
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
