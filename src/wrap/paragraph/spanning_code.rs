//! Source-line preservation for inline code spans that cannot fit atomically.
//!
//! This module owns the narrow fallback used by paragraph flushing: source
//! boundaries may be retained only when they occur inside an inline-code span,
//! every authored line already fits, and joining the span would exceed the
//! configured width. Ordinary prose remains owned by the greedy wrapper.

use unicode_width::UnicodeWidthStr;

use crate::wrap::tokenize::{has_odd_backslash_escape_bytes, position_after_close};

pub(super) fn conforming_source_lines_for_overlong_span(
    segments: &[(String, bool)],
    width: usize,
) -> Option<Vec<String>> {
    if segments.len() < 2 || segments.iter().any(|(line, _)| line.width() > width) {
        return None;
    }

    let (joined, boundaries) = join_with_boundaries(segments);
    if !has_overlong_code_span_crossing_boundary(&joined, &boundaries, width) {
        return None;
    }

    Some(
        segments
            .iter()
            .map(|(line, hard_break)| restore_hard_break(line, *hard_break))
            .collect(),
    )
}

fn join_with_boundaries(segments: &[(String, bool)]) -> (String, Vec<usize>) {
    let mut joined = String::new();
    let mut boundaries = Vec::with_capacity(segments.len().saturating_sub(1));
    for (index, (line, _)) in segments.iter().enumerate() {
        if index > 0 {
            boundaries.push(joined.len());
            joined.push(' ');
        }
        joined.push_str(line);
    }
    (joined, boundaries)
}

fn has_overlong_code_span_crossing_boundary(
    text: &str,
    boundaries: &[usize],
    width: usize,
) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < text.len() {
        let Some(ch) = text[index..].chars().next() else {
            break;
        };
        if ch != '`' || has_odd_backslash_escape_bytes(bytes, index) {
            index += ch.len_utf8();
            continue;
        }

        let fence_len = text[index..]
            .chars()
            .take_while(|candidate| *candidate == '`')
            .count();
        let fence_end = index + fence_len;
        let Some(close_end) = position_after_close(text, fence_end, fence_len) else {
            return false;
        };
        let crosses_boundary = boundaries
            .iter()
            .any(|boundary| index < *boundary && *boundary < close_end);
        if crosses_boundary && text[index..close_end].width() > width {
            return true;
        }
        index = close_end;
    }
    false
}

fn restore_hard_break(line: &str, hard_break: bool) -> String {
    if hard_break && !line.ends_with("  ") {
        format!("{line}  ")
    } else {
        line.to_string()
    }
}
