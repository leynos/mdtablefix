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

use tracing::trace;

mod block;
mod continuation;
mod fence;
mod inline;
mod link_reference;
mod paragraph;
mod tokenize;
use block::{BLOCKQUOTE_RE, BULLET_RE, FOOTNOTE_RE};
pub(crate) use block::{BlockKind, classify_block, leading_indent};
use continuation::apply_continuation_chunk;
/// Fence-detection utilities re-exported for downstream callers.
///
/// [`FenceTracker`] maintains fenced code-block state across lines, which is
/// useful for callers that process Markdown incrementally. [`is_fence`]
/// inspects one line and returns the fence components (indentation, marker,
/// info string) when the line opens a fenced code block, or `None` otherwise.
pub use fence::{FenceTracker, is_fence};
pub(crate) use link_reference::{LinkReferenceMatcher, LinkTitleWindow, LinkTitleWindowOutcome};
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
pub(crate) use tokenize::link_or_image_span;
#[doc(inline)]
pub use tokenize::tokenize_markdown;
// Re-exported for unit tests; not used in production code.
#[cfg(test)]
pub(crate) use tokenize::{continuation_begins_with_closing_fence, has_unclosed_code_span};

// Permit GFM task list markers with flexible spacing and missing post-marker
// spaces in Markdown.

fn is_indented_code_line(line: &str) -> bool {
    let (indent_width, first_content_byte) = leading_indent(line);
    indent_width >= 4
        && line[first_content_byte..]
            .chars()
            .any(|c| !c.is_whitespace())
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
        let prefix = cap.get(1).map(|m| m.as_str())?;
        let rest = cap.get(2).map(|m| m.as_str())?;
        return Some(PrefixLine {
            prefix: Cow::Borrowed(prefix),
            rest,
            repeat_prefix: false,
        });
    }

    if let Some(cap) = FOOTNOTE_RE.captures(line) {
        let prefix = cap.get(1).map(|m| m.as_str())?;
        let marker = cap.get(2).map(|m| m.as_str())?;
        let rest = cap.get(3).map(|m| m.as_str())?;
        return Some(PrefixLine {
            prefix: Cow::Owned(format!("{prefix}{marker}")),
            rest,
            repeat_prefix: false,
        });
    }

    let Some(cap) = BLOCKQUOTE_RE.captures(line) else {
        trace!(
            line_len = line.len(),
            "prefix_line found no supported prefix"
        );
        return None;
    };
    let prefix = cap.get(1).map(|m| m.as_str())?;
    let rest = cap.get(2).map(|m| m.as_str())?;
    Some(PrefixLine {
        prefix: Cow::Borrowed(prefix),
        rest,
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

fn normalized_passthrough_line(line: &str) -> &str {
    if !line.is_empty() && line.trim().is_empty() {
        trace!(
            line_len = line.len(),
            "normalizing whitespace-only passthrough line"
        );
        ""
    } else {
        line
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
            apply_continuation_chunk(&text, line, hard_break, writer, state);
            return;
        }

        writer.handle_prefix_line(state, &prefix_line);
        return;
    }

    if is_passthrough_block(block_kind, line) {
        if matches!(block_kind, Some(BlockKind::LinkReferenceDefinition)) {
            link_title_window.observe_definition(line, link_matcher);
        }
        let emitted = normalized_passthrough_line(line);
        writer.push_verbatim(state, emitted);
        return;
    }

    let (text, hard_break) = line_break_parts(line);
    if state.pending_prefix.is_none() {
        return;
    }
    apply_continuation_chunk(&text, line, hard_break, writer, state);
}

/// Wrap text lines to the given width.
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
            if matches!(block_kind, Some(BlockKind::LinkReferenceDefinition)) {
                link_title_window.observe_definition(line, link_matcher);
            }
            // Whitespace-only lines act as paragraph breaks; emit them as empty
            // strings so downstream consumers see a uniform separator.
            let emitted = normalized_passthrough_line(line);
            writer.push_verbatim(&mut state, emitted);
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
