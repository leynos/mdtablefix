//! Source-line preservation for inline code spans that cannot fit atomically.
//!
//! This module owns the narrow fallback used by paragraph flushing: source
//! boundaries may be retained only when they occur inside an inline-code span,
//! every authored line already fits, and joining the span would exceed the
//! configured width. Ordinary prose remains owned by the greedy wrapper.

use tracing::trace;
use unicode_width::UnicodeWidthStr;

use crate::wrap::{
    inline::wrap_preserving_code,
    tokenize::{has_odd_backslash_escape_bytes, position_after_close},
};

#[derive(Debug)]
struct OverlongSpan {
    text: String,
    pieces: Vec<String>,
}

pub(super) fn conforming_source_lines_for_overlong_span(
    segments: &[(String, bool)],
    indent: &str,
    width: usize,
) -> Option<Vec<String>> {
    let indent_width = indent.width();
    let available = width.saturating_sub(indent_width).max(1);
    if segments.len() < 2 || segments.iter().any(|(line, _)| line.width() > available) {
        return None;
    }

    let mut found_overlong_span = false;
    let mut output = Vec::new();
    for group in hard_break_groups(segments) {
        let (joined, boundaries) = join_with_boundaries(group);
        let spans = overlong_code_spans_crossing_boundaries(&joined, &boundaries, available);
        found_overlong_span |= !spans.is_empty();

        let mut lines = wrap_preserving_code(&joined, available);
        for span in spans {
            preserve_span_boundaries(&mut lines, &span, available);
        }
        if group.last().is_some_and(|(_, hard_break)| *hard_break) {
            restore_last_hard_break(&mut lines);
        }
        output.extend(lines.into_iter().map(|line| format!("{indent}{line}")));
    }

    if !found_overlong_span {
        return None;
    }
    trace!(
        mode = "preserve_authored_boundaries",
        width,
        boundary = "inline_code",
        line_count = output.len(),
        "preserved authored boundaries inside an overlong inline-code span"
    );
    Some(output)
}

fn hard_break_groups(segments: &[(String, bool)]) -> impl Iterator<Item = &[(String, bool)]> {
    let mut start = 0;
    std::iter::from_fn(move || {
        if start == segments.len() {
            return None;
        }
        let relative_end = segments[start..]
            .iter()
            .position(|(_, hard_break)| *hard_break)
            .map_or(segments.len(), |index| start + index + 1);
        let group = &segments[start..relative_end];
        start = relative_end;
        Some(group)
    })
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

fn overlong_code_spans_crossing_boundaries(
    text: &str,
    boundaries: &[usize],
    width: usize,
) -> Vec<OverlongSpan> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut spans = Vec::new();
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
            trace!(
                mode = "spanning_code_scan",
                boundary = "unmatched_fence",
                fence_len,
                "continued scanning after an unmatched inline-code fence"
            );
            index = fence_end;
            continue;
        };
        let span_boundaries = boundaries
            .iter()
            .copied()
            .filter(|boundary| index < *boundary && *boundary < close_end)
            .collect::<Vec<_>>();
        if !span_boundaries.is_empty() && text[index..close_end].width() > width {
            spans.push(OverlongSpan {
                text: text[index..close_end].to_string(),
                pieces: split_span_at_boundaries(text, index, close_end, &span_boundaries),
            });
        }
        index = close_end;
    }
    spans
}

fn split_span_at_boundaries(
    text: &str,
    start: usize,
    end: usize,
    boundaries: &[usize],
) -> Vec<String> {
    let mut pieces = Vec::with_capacity(boundaries.len() + 1);
    let mut piece_start = start;
    for boundary in boundaries {
        pieces.push(text[piece_start..*boundary].to_string());
        piece_start = boundary + 1;
    }
    pieces.push(text[piece_start..end].to_string());
    pieces
}

fn preserve_span_boundaries(lines: &mut Vec<String>, span: &OverlongSpan, width: usize) {
    let Some((line_index, span_start)) = lines
        .iter()
        .enumerate()
        .find_map(|(index, line)| line.find(&span.text).map(|offset| (index, offset)))
    else {
        return;
    };
    let line = lines.remove(line_index);
    let span_end = span_start + span.text.len();
    let before = &line[..span_start];
    let after = &line[span_end..];
    let mut replacement = span.pieces.clone();

    prepend_prose(&mut replacement, before, width);
    append_prose(&mut replacement, after, width);
    lines.splice(line_index..line_index, replacement);
}

fn prepend_prose(lines: &mut Vec<String>, before: &str, width: usize) {
    if before.is_empty() {
        return;
    }
    let combined = format!("{before}{}", lines[0]);
    if combined.width() <= width {
        lines[0] = combined;
        return;
    }
    let mut prose = wrap_preserving_code(before.trim_end(), width);
    prose.append(lines);
    *lines = prose;
}

fn append_prose(lines: &mut Vec<String>, after: &str, width: usize) {
    if after.is_empty() {
        return;
    }
    let last_index = lines.len() - 1;
    let combined = format!("{}{after}", lines[last_index]);
    if combined.width() <= width {
        lines[last_index] = combined;
        return;
    }
    lines.extend(wrap_preserving_code(after.trim_start(), width));
}

fn restore_last_hard_break(lines: &mut [String]) {
    if let Some(line) = lines.last_mut()
        && !line.ends_with("  ")
    {
        line.push_str("  ");
    }
}
