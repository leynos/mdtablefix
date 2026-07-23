//! Inline code-span edge-space trimming for pending prefixed paragraphs.
//!
//! This module trims only the leading and trailing spaces inside complete code
//! spans, preserving the exact fence length so literal shorter backtick runs
//! remain untouched as span content.

use std::{borrow::Cow, collections::HashSet};

use tracing::trace;

pub(super) fn trim_code_span_edge_spaces<'a>(
    text: &'a str,
    synthetic_spaces: &[usize],
) -> Cow<'a, str> {
    if synthetic_spaces.is_empty() || (!text.contains("` ") && !text.contains(" `")) {
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
            code_end > code_start
                && synthetic_space_offsets.contains(&(code_end.saturating_sub(1))),
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
        output.push_str(&remaining[..open_end]);
        output.push_str(&remaining[open_end + trim_start..close_start - trim_end]);
        output.push_str(&remaining[close_start..close_end]);
        remaining = &remaining[close_end..];
        consumed += close_end;
    }
    output.push_str(remaining);
    Cow::Owned(output)
}

fn next_backtick_run(text: &str, start: usize) -> Option<(usize, usize)> {
    let mut index = start;
    while index < text.len() {
        let ch = text[index..].chars().next()?;
        if ch == '`' && !has_odd_backslash_escape(text.as_bytes(), index) {
            return Some((index, backtick_run_end(text, index)));
        }
        index += ch.len_utf8();
    }
    None
}

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

fn is_exact_backtick_run(text: &str, start: usize, end: usize, fence_len: usize) -> bool {
    end - start == fence_len
        && start
            .checked_sub(1)
            .is_none_or(|before| text.as_bytes()[before] != b'`')
        && text.as_bytes().get(end).is_none_or(|next| *next != b'`')
}

fn backtick_run_end(text: &str, start: usize) -> usize {
    let mut end = start;
    for ch in text[start..].chars() {
        if ch != '`' {
            break;
        }
        end += ch.len_utf8();
    }
    end
}

fn has_odd_backslash_escape(bytes: &[u8], mut index: usize) -> bool {
    let mut count = 0;
    while index > 0 {
        index -= 1;
        if bytes[index] != b'\\' {
            break;
        }
        count += 1;
    }
    count % 2 == 1
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
}
