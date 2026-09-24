//! Source-line preservation for inline code spans that cannot fit atomically.
//!
//! This module owns the narrow fallback used by paragraph flushing: source
//! boundaries may be retained only when they occur inside an inline-code span,
//! every authored line already fits, and joining the span would exceed the
//! configured width. Ordinary prose remains owned by the greedy wrapper.

use std::ops::Range;

use tracing::trace;
use unicode_width::UnicodeWidthStr;

use super::hard_break::trailing_hard_break_marker_len;
use crate::wrap::{
    inline::wrap_preserving_code,
    tokenize::{has_odd_backslash_escape_bytes, position_after_close},
};

/// A complete inline-code span whose authored line boundaries must be restored.
#[derive(Debug)]
struct OverlongSpan {
    /// Byte range of the complete fenced span in the joined source text.
    range: Range<usize>,
    /// Pieces split at authored line boundaries, excluding join spaces.
    pieces: Vec<String>,
}

/// Which preserved span edge receives reattached prose.
#[derive(Clone, Copy)]
enum ProseEdge {
    /// The edge before the first preserved span piece.
    Leading,
    /// The edge after the last preserved span piece.
    Trailing,
}

impl ProseEdge {
    /// Join prose beside this preserved edge only when it fits one line.
    fn join(self, lines: &[String], prose: &str, width: usize) -> Option<String> {
        match self {
            Self::Leading => join_if_fits(prose, lines.first()?, width),
            Self::Trailing => join_if_fits(lines.last()?, prose, width),
        }
    }

    /// Replace the preserved piece at this edge.
    fn replace(self, lines: &mut [String], joined: String) {
        let target = match self {
            Self::Leading => lines.first_mut(),
            Self::Trailing => lines.last_mut(),
        };
        if let Some(target) = target {
            *target = joined;
        }
    }

    /// Wrap and position prose when it does not fit beside this edge.
    fn wrap_and_attach(self, lines: &mut Vec<String>, prose: &str, width: usize) {
        let trimmed = match self {
            Self::Leading => prose.trim_end(),
            Self::Trailing => prose.trim_start(),
        };
        let mut wrapped = wrap_preserving_code(trimmed, width);
        match self {
            Self::Leading => {
                wrapped.append(lines);
                *lines = wrapped;
            }
            Self::Trailing => lines.append(&mut wrapped),
        }
    }
}

/// Formats conforming source lines when an inline-code span must stay split.
///
/// `segments` contains the buffered source text and hard-break markers,
/// `indent` is the leading whitespace restored on every emitted line, and
/// `width` is the total display-column limit including that indent. The helper
/// returns `Some` formatted lines only when an overlong inline-code span crosses
/// authored boundaries. Otherwise it returns `None`, deferring emission to the
/// ordinary greedy wrapper.
///
/// Detection runs before greedy wrapping. For `T` joined text bytes, `F` fence
/// runs, and `B` authored boundaries, its worst case is `O(T × F + F log B)`.
/// For `S` qualifying spans and `L` wrapped output bytes, the fallback adds
/// `O(S × L)` output searches; wrapping and those searches run only after a
/// qualifying span is found.
pub(super) fn conforming_source_lines_for_overlong_span(
    segments: &[(String, bool)],
    indent: &str,
    width: usize,
) -> Option<Vec<String>> {
    let indent_width = indent.width();
    let available = width.saturating_sub(indent_width).max(1);
    if segments.len() < 2 {
        return None;
    }

    let groups = hard_break_groups(segments)
        .map(|group| {
            let (joined, boundaries) = join_with_boundaries(group);
            let spans = if group.len() < 2 || group.iter().any(|(line, _)| line.width() > available)
            {
                Vec::new()
            } else {
                overlong_code_spans_crossing_boundaries(&joined, &boundaries, available)
            };
            let has_hard_break = group.last().is_some_and(|(_, hard_break)| *hard_break);
            (joined, spans, has_hard_break)
        })
        .collect::<Vec<_>>();
    let found_overlong_span = groups.iter().any(|(_, spans, _)| !spans.is_empty());
    if !found_overlong_span {
        return None;
    }

    let mut output = Vec::new();
    for (joined, spans, has_hard_break) in groups {
        let mut lines = wrap_preserving_code(&joined, available);
        for span in spans {
            preserve_span_boundaries(&mut lines, &joined, span, available);
        }
        if has_hard_break {
            restore_last_hard_break(&mut lines);
        }
        output.extend(lines.into_iter().map(|line| format!("{indent}{line}")));
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

/// Group source segments at authored hard breaks while retaining each group as
/// one slice of the original segment array.
///
/// A hard break belongs to the group that ends with it, so later formatting can
/// restore its marker after preserving any code-span boundaries.
fn hard_break_groups(segments: &[(String, bool)]) -> impl Iterator<Item = &[(String, bool)]> {
    let mut start = 0;
    std::iter::from_fn(move || {
        if start == segments.len() {
            return None;
        }
        let remaining = segments.get(start..)?;
        let relative_end = remaining
            .iter()
            .position(|(_, hard_break)| *hard_break)
            .map_or(segments.len(), |index| start + index + 1);
        let group = segments.get(start..relative_end)?;
        start = relative_end;
        Some(group)
    })
}

/// Join source segments with synthetic spaces and record each insertion point.
///
/// The recorded offsets identify boundaries that may be restored inside an
/// overlong code span; the inserted spaces themselves are never retained.
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

/// Find inline-code spans that cross authored boundaries and exceed `width`.
///
/// Only complete, unescaped spans qualify. Unmatched fences are skipped so a
/// malformed opener cannot cause arbitrary prose boundaries to be preserved.
fn overlong_code_spans_crossing_boundaries(
    text: &str,
    boundaries: &[usize],
    width: usize,
) -> Vec<OverlongSpan> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut spans = Vec::new();
    while index < text.len() {
        let Some(suffix) = text.get(index..) else {
            break;
        };
        let Some(ch) = suffix.chars().next() else {
            break;
        };
        if ch != '`' || has_odd_backslash_escape_bytes(bytes, index) {
            index += ch.len_utf8();
            continue;
        }

        let fence_len = suffix
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
        if let Some(span) =
            overlong_span_crossing_boundaries(text, boundaries, index..close_end, width)
        {
            spans.push(span);
        }
        index = close_end;
    }
    spans
}

/// Build a candidate only when its complete code span crosses an authored
/// boundary and exceeds the display width.
fn overlong_span_crossing_boundaries(
    text: &str,
    boundaries: &[usize],
    range: Range<usize>,
    width: usize,
) -> Option<OverlongSpan> {
    let first_boundary = boundaries.partition_point(|boundary| *boundary <= range.start);
    let last_boundary = boundaries.partition_point(|boundary| *boundary < range.end);
    let span_boundaries = boundaries.get(first_boundary..last_boundary)?;
    let span_text = text.get(range.clone())?;
    if span_boundaries.is_empty() || span_text.width() <= width {
        return None;
    }
    let pieces = split_span_at_boundaries(text, range.start, range.end, span_boundaries)?;
    Some(OverlongSpan { range, pieces })
}

/// Split a complete code span at the supplied joined-text boundaries.
///
/// Each boundary represents one synthetic space inserted by
/// [`join_with_boundaries`], so that space is omitted from the returned pieces.
fn split_span_at_boundaries(
    text: &str,
    start: usize,
    end: usize,
    boundaries: &[usize],
) -> Option<Vec<String>> {
    let mut pieces = Vec::with_capacity(boundaries.len() + 1);
    let mut piece_start = start;
    for boundary in boundaries {
        pieces.push(text.get(piece_start..*boundary)?.to_owned());
        piece_start = boundary.checked_add(1)?;
    }
    pieces.push(text.get(piece_start..end)?.to_owned());
    Some(pieces)
}

/// Replace a wrapped overlong span with pieces that retain authored breaks.
///
/// Prose before and after the span is reattached only when it still fits; if it
/// does not, it is wrapped independently around the preserved pieces.
fn preserve_span_boundaries(
    lines: &mut Vec<String>,
    joined: &str,
    span: OverlongSpan,
    width: usize,
) {
    let OverlongSpan { range, pieces } = span;
    let Some(span_text) = joined.get(range) else {
        return;
    };
    let Some((line_index, span_start)) = lines
        .iter()
        .enumerate()
        .find_map(|(index, line)| line.find(span_text).map(|offset| (index, offset)))
    else {
        trace!(
            mode = "preserve_authored_boundaries",
            width,
            boundary = "span_lookup_miss",
            line_count = lines.len(),
            span_width = span_text.width(),
            "skipped authored boundaries because the wrapped span was not found"
        );
        return;
    };
    let line = lines.remove(line_index);
    let span_end = span_start + span_text.len();
    let (Some(before), Some(after)) = (line.get(..span_start), line.get(span_end..)) else {
        lines.insert(line_index, line);
        return;
    };
    let mut replacement = pieces;

    reattach_prose(&mut replacement, before, width, ProseEdge::Leading);
    reattach_prose(&mut replacement, after, width, ProseEdge::Trailing);
    lines.splice(line_index..line_index, replacement);
}

/// Reattach prose at one preserved span edge; private to [`preserve_span_boundaries`].
///
/// It composes [`join_if_fits`] with the greedy wrapper, retaining each edge's
/// join order, trim direction, and placement.
fn reattach_prose(lines: &mut Vec<String>, prose: &str, width: usize, edge: ProseEdge) {
    if prose.is_empty() || lines.is_empty() {
        return;
    }
    if let Some(joined) = edge.join(lines, prose, width) {
        edge.replace(lines, joined);
        return;
    }
    edge.wrap_and_attach(lines, prose, width);
}

/// Join adjacent source fragments only when their display width fits one line.
fn join_if_fits(prefix: &str, suffix: &str, width: usize) -> Option<String> {
    let joined = format!("{prefix}{suffix}");
    (joined.width() <= width).then_some(joined)
}

/// Restore the two-space Markdown hard-break marker after span reflow.
fn restore_last_hard_break(lines: &mut [String]) {
    if let Some(line) = lines.last_mut()
        && trailing_hard_break_marker_len(line) == 0
    {
        line.push_str("  ");
    }
}

#[cfg(test)]
mod tests {
    //! Exact-output checks for source boundaries inside overlong code spans.

    use super::{
        ProseEdge,
        conforming_source_lines_for_overlong_span,
        join_with_boundaries,
        reattach_prose,
    };

    #[test]
    fn preserves_byte_boundary_after_multibyte_code_content() {
        let segments = [("`éab".to_owned(), false), ("cd`".to_owned(), false)];
        let (joined, boundaries) = join_with_boundaries(&segments);

        assert_eq!(joined, "`éab cd`");
        assert_eq!(boundaries, [5]);
        assert_eq!(
            conforming_source_lines_for_overlong_span(&segments, "", 4),
            Some(vec!["`éab".to_owned(), "cd`".to_owned()]),
        );
    }

    #[test]
    fn preserves_boundary_immediately_after_multibyte_code_content() {
        let segments = [("`é".to_owned(), false), ("abc`".to_owned(), false)];
        let (joined, boundaries) = join_with_boundaries(&segments);

        assert_eq!(joined, "`é abc`");
        assert_eq!(boundaries, [3]);
        assert_eq!(
            conforming_source_lines_for_overlong_span(&segments, "", 4),
            Some(vec!["`é".to_owned(), "abc`".to_owned()]),
        );
    }

    #[test]
    fn joins_prose_at_the_requested_preserved_span_edge() {
        let mut leading = vec!["`code`".to_owned()];
        reattach_prose(&mut leading, "before ", 20, ProseEdge::Leading);
        assert_eq!(leading, ["before `code`"]);

        let mut trailing = vec!["`code`".to_owned()];
        reattach_prose(&mut trailing, " after", 20, ProseEdge::Trailing);
        assert_eq!(trailing, ["`code` after"]);
    }

    #[test]
    fn wraps_trimmed_prose_at_the_requested_preserved_span_edge() {
        let mut leading = vec!["`code".to_owned(), "span`".to_owned()];
        reattach_prose(&mut leading, "before ", 5, ProseEdge::Leading);
        assert_eq!(leading, ["before", "`code", "span`"]);

        let mut trailing = vec!["`code".to_owned(), "span`".to_owned()];
        reattach_prose(&mut trailing, " after", 5, ProseEdge::Trailing);
        assert_eq!(trailing, ["`code", "span`", "after"]);
    }
}
