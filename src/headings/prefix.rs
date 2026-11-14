//! Prefix and indentation measurement for Setext heading candidates.
//!
//! A Setext pair is admitted only when its two lines agree about the prefix in
//! front of their content: the same indentation and the same blockquote depth.
//! These helpers measure that prefix so the heading pass can compare the text
//! line with its underline, and so it can reject a pair indented far enough to
//! be an indented code block instead.
//!
//! Everything here reads a single line and returns a width, an offset, or a
//! slice. Nothing here decides whether a heading is converted; the caller owns
//! that admission and this module only measures what the decision needs.

use crate::{classify::quote_depth, wrap::leading_indent};

/// Returns the indentation width of a line's content, in columns.
///
/// Blockquote markers are consumed before the width is measured, so
/// `>     code` counts as four columns inside the quote. A marker may itself be
/// reached through one to three leading spaces, and those spaces belong to the
/// marker rather than to the content: `   >     code` is four columns too,
/// which is what makes it an indented code block. Tabs count as four columns,
/// matching [`crate::wrap::leading_indent`].
pub(super) fn content_indent_width(line: &str) -> usize {
    let mut rest = line;

    while let Some(tail) = strip_blockquote_marker(rest) {
        rest = tail;
    }

    leading_indent(rest).0
}

/// Strips one blockquote marker, returning the content that follows it.
///
/// A marker is up to three leading spaces, a `>`, and the single space that may
/// follow it. The wrapper's blockquote prefix accepts the same shape, so both
/// passes measure the quoted content from the same point. Four or more leading
/// spaces are an indented code block rather than a marker, and are left in
/// place for the caller to measure.
fn strip_blockquote_marker(line: &str) -> Option<&str> {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count().min(3);
    let marker = line.get(indent..)?.strip_prefix('>')?;
    Some(marker.strip_prefix(' ').unwrap_or(marker))
}

/// Returns the byte length of the character-aligned prefix shared by two lines.
///
/// The result always falls on a character boundary of both lines, so callers
/// may slice either one at that offset.
pub(super) fn shared_prefix_len(a: &str, b: &str) -> usize {
    let mut end = 0;
    let mut iter_a = a.char_indices();
    let mut iter_b = b.char_indices();

    loop {
        match (iter_a.next(), iter_b.next()) {
            (Some((idx_a, ch_a)), Some((_, ch_b))) if ch_a == ch_b => {
                end = idx_a + ch_a.len_utf8();
            }
            _ => break,
        }
    }

    end
}

/// Determine whether a line and its underline disagree on blockquote depth.
///
/// Setext headings must remain within one quote level. Up to three spaces of
/// indentation may differ between the text and underline; the classifier
/// independently rejects indented code.
pub(super) fn has_unmatched_prefix(line: &str, underline: &str) -> bool {
    let line_prefix = prefix_of_indent_or_quote(line);
    let underline_prefix = prefix_of_indent_or_quote(underline);
    let line_depth = quote_depth(&line[..line_prefix]);
    let underline_depth = quote_depth(&underline[..underline_prefix]);
    line_depth != underline_depth
}

/// Returns the byte length of leading indentation and blockquote markers.
fn prefix_of_indent_or_quote(text: &str) -> usize {
    let mut last = 0;
    for (idx, ch) in text.char_indices() {
        if ch.is_whitespace() || ch == '>' {
            last = idx + ch.len_utf8();
            continue;
        }
        break;
    }
    last
}
