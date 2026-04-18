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
mod line_buffer;
mod paragraph;
mod tokenize;
use block::{BLOCKQUOTE_RE, BULLET_RE, FOOTNOTE_RE};
pub(crate) use block::{BlockKind, classify_block};
pub use fence::{FenceTracker, is_fence};
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
#[doc(inline)]
pub use tokenize::tokenize_markdown;

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

fn is_passthrough_block(line: &str) -> bool {
    is_table_or_separator(line)
        || matches!(
            classify_block(line),
            Some(BlockKind::Heading | BlockKind::MarkdownlintDirective)
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

    for line in lines {
        if fence::handle_fence_line(line, &mut writer, &mut state, &mut fence_tracker) {
            continue;
        }

        if fence_tracker.in_fence() {
            writer.push_verbatim(&mut state, line);
            continue;
        }

        if is_passthrough_block(line) {
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
