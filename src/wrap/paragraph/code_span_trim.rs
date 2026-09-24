//! Inline code-span edge-space trimming for pending prefixed paragraphs.
//!
//! This module trims only the leading and trailing spaces inside complete code
//! spans, preserving the exact fence length so literal shorter backtick runs
//! remain untouched as span content.

use std::{borrow::Cow, collections::HashSet};

use tracing::trace;

/// Trim only synthetic spaces at the edges of complete inline code spans.
///
/// `synthetic_spaces` contains offsets introduced while joining prefixed lines;
/// authored spaces remain intact. A span is edited only when its closing run
/// has exactly the opener's length, so shorter literal runs stay content.
pub(super) fn trim_code_span_edge_spaces<'a>(
    text: &'a str,
    synthetic_spaces: &[usize],
) -> Cow<'a, str> {
    if !has_potential_synthetic_edge_space(text, synthetic_spaces) {
        return Cow::Borrowed(text);
    }

    let mut output = String::with_capacity(text.len());
    let mut remaining = text;
    let mut consumed = 0;
    let synthetic_space_offsets: HashSet<usize> = synthetic_spaces.iter().copied().collect();
    while let Some((open_start, open_end)) = next_backtick_run(remaining, 0) {
        let fence_len = open_end - open_start;
        let Some(close_start) = matching_backtick_run_start(remaining, open_end, fence_len) else {
            output.push_str(remaining);
            return Cow::Owned(output);
        };
        let close_end = close_start + fence_len;
        let code_start = consumed + open_end;
        let code_end = consumed + close_start;
        let trim_start = usize::from(synthetic_space_offsets.contains(&code_start));
        let trim_end = usize::from(
            (code_start..code_end)
                .next_back()
                .is_some_and(|last| synthetic_space_offsets.contains(&last)),
        );
        if trim_start > 0 || trim_end > 0 {
            trace!(
                fence_len,
                code_start,
                code_end,
                trim_start,
                trim_end,
                "trimmed synthetic code-span edge spaces"
            );
        }
        let Some(trimmed_end) = close_start.checked_sub(trim_end) else {
            return Cow::Borrowed(text);
        };
        let (Some(opener), Some(content), Some(closer), Some(rest)) = (
            remaining.get(..open_end),
            remaining.get(open_end + trim_start..trimmed_end),
            remaining.get(close_start..close_end),
            remaining.get(close_end..),
        ) else {
            return Cow::Borrowed(text);
        };
        output.push_str(opener);
        output.push_str(content);
        output.push_str(closer);
        remaining = rest;
        consumed += close_end;
    }
    output.push_str(remaining);
    Cow::Owned(output)
}

/// Limit the trim scan to joined lines that might have a synthetic code edge.
fn has_potential_synthetic_edge_space(text: &str, synthetic_spaces: &[usize]) -> bool {
    !synthetic_spaces.is_empty() && (text.contains("` ") || text.contains(" `"))
}

/// Find the next unescaped backtick run beginning at or after `start`.
fn next_backtick_run(text: &str, start: usize) -> Option<(usize, usize)> {
    let mut index = start;
    while index < text.len() {
        let ch = text.get(index..)?.chars().next()?;
        if ch == '`' && !has_odd_backslash_escape(text.as_bytes(), index) {
            return Some((index, backtick_run_end(text, index)));
        }
        index += ch.len_utf8();
    }
    None
}

/// Find the first candidate closing run with the opener's exact fence length.
fn matching_backtick_run_start(text: &str, start: usize, fence_len: usize) -> Option<usize> {
    let mut search = start;
    while let Some((run_start, run_end)) = next_backtick_run(text, search) {
        if is_exact_backtick_run(text, run_start, run_end, fence_len) {
            return Some(run_start);
        }
        search = run_end;
    }
    None
}

/// Return whether a backtick run is isolated from adjacent backticks.
///
/// Isolation prevents treating part of a longer run as a valid closing fence.
fn is_exact_backtick_run(text: &str, start: usize, end: usize, fence_len: usize) -> bool {
    end - start == fence_len
        && start
            .checked_sub(1)
            .is_none_or(|before| text.as_bytes().get(before) != Some(&b'`'))
        && text.as_bytes().get(end).is_none_or(|next| *next != b'`')
}

/// Return the byte offset immediately after a contiguous backtick run.
fn backtick_run_end(text: &str, start: usize) -> usize {
    let mut end = start;
    let Some(suffix) = text.get(start..) else {
        return start;
    };
    for ch in suffix.chars() {
        if ch != '`' {
            break;
        }
        end += ch.len_utf8();
    }
    end
}

/// Return whether the byte at `index` has an odd backslash escape prefix.
///
/// Odd parity means the backtick is literal; even parity leaves it eligible as
/// a code-span delimiter.
fn has_odd_backslash_escape(bytes: &[u8], mut index: usize) -> bool {
    let mut count = 0usize;
    while index > 0 {
        index -= 1;
        if bytes.get(index) != Some(&b'\\') {
            break;
        }
        count += 1;
    }
    count.rem_euclid(2) == 1
}

#[cfg(test)]
mod tests {
    //! Unit tests for code-span edge-space trimming.

    use std::borrow::Cow;

    use super::trim_code_span_edge_spaces;

    #[test]
    fn trims_synthetic_single_backtick_span_edge_spaces() {
        assert_eq!(
            trim_code_span_edge_spaces("` foo `", &[1, 5]),
            Cow::Borrowed("`foo`"),
        );
    }

    #[test]
    fn preserves_authored_edge_spaces_without_synthetic_metadata() {
        assert_eq!(
            trim_code_span_edge_spaces("calls ` foo ` now", &[]),
            Cow::Borrowed("calls ` foo ` now"),
        );
    }

    #[test]
    fn respects_multi_backtick_fences() {
        assert_eq!(
            trim_code_span_edge_spaces("`` foo ` bar ` baz ``", &[2, 18]),
            Cow::Borrowed("``foo ` bar ` baz``"),
        );
    }

    #[test]
    fn trims_synthetic_spaces_after_multibyte_prefix() {
        assert_eq!(
            trim_code_span_edge_spaces("é ` foo ` 末", &[4, 8]),
            Cow::Borrowed("é `foo` 末"),
        );
    }
}
