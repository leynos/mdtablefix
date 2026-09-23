//! Character-sequence specifications for the production scanner predicates.

use vstd::prelude::*;
use crate::production_classify;

verus! {

/// The two scalar values that Markdown treats as indentation whitespace.
pub open spec fn spec_is_markdown_whitespace(character: char) -> bool {
    character == ' ' || character == '\t'
}

/// Whether the suffix beginning at `start` contains only indentation whitespace.
pub open spec fn spec_is_blank_from(s: Seq<char>, start: int) -> bool {
    forall|i: int| start <= i < s.len() ==> spec_is_markdown_whitespace(s[i])
}

/// Whether a bounded scalar range contains the requested marker.
pub open spec fn spec_contains(s: Seq<char>, start: int, end: int, target: char) -> bool {
    exists|i: int| start <= i < end && s[i] == target
}

/// Whether every scalar in a bounded range is the same marker.
pub open spec fn spec_all_equal(s: Seq<char>, start: int, end: int, target: char) -> bool {
    forall|i: int| start <= i < end ==> s[i] == target
}

/// First non-whitespace scalar in a bounded range, or its end.
pub open spec fn spec_trim_start(s: Seq<char>, start: int, end: int) -> int
    recommends 0 <= start <= end <= s.len()
    decreases end - start
{
    if start < end && spec_is_markdown_whitespace(s[start]) {
        spec_trim_start(s, start + 1, end)
    } else {
        start
    }
}

/// End of a bounded range after removing trailing whitespace.
pub open spec fn spec_trim_end(s: Seq<char>, start: int, end: int) -> int
    recommends 0 <= start <= end <= s.len()
    decreases end - start
{
    if start < end && spec_is_markdown_whitespace(s[end - 1]) {
        spec_trim_end(s, start, end - 1)
    } else {
        end
    }
}

/// Length of the uninterrupted marker run from `start` within a bounded range.
pub open spec fn spec_marker_run_len(s: Seq<char>, start: int, end: int, marker: char) -> int
    recommends 0 <= start <= end <= s.len()
    decreases end - start
{
    if start < end && s[start] == marker {
        1 + spec_marker_run_len(s, start + 1, end, marker)
    } else {
        0
    }
}

/// Shared trimmed body range for structural predicates.
pub open spec fn spec_trimmed_range(s: Seq<char>, start: int) -> (int, int)
    recommends 0 <= start <= s.len()
{
    let first = spec_trim_start(s, start, s.len() as int);
    (first, spec_trim_end(s, first, s.len() as int))
}

/// Fence-marker recognition over a trimmed character sequence.
pub open spec fn spec_fence_marker(s: Seq<char>, start: int) -> bool
    recommends 0 <= start <= s.len()
{
    let (first, end) = spec_trimmed_range(s, start);
    first < s.len()
        && (s[first] == '`' || s[first] == '~')
        && spec_marker_run_len(s, first, end, s[first]) >= 3
}

/// Compatible closing marker for an existing fence.
pub open spec fn spec_closing_fence(
    s: Seq<char>, start: int, open: production_classify::OpenFence,
) -> bool
    recommends 0 <= start <= s.len()
{
    let (first, end) = spec_trimmed_range(s, start);
    let markers = spec_marker_run_len(s, first, end, open.marker);
    markers >= open.marker_len && markers == end - first
}

/// ATX marker recognition with CommonMark's six-hash limit.
pub open spec fn spec_atx_heading(s: Seq<char>, start: int) -> bool
    recommends 0 <= start <= s.len()
{
    let (first, end) = spec_trimmed_range(s, start);
    let hashes = spec_marker_run_len(s, first, end, '#');
    hashes > 0 && hashes <= 6
        && (first + hashes == end || spec_is_markdown_whitespace(s[first + hashes]))
}

/// Whether the structural body begins with a pipe.
pub open spec fn spec_body_starts_with_pipe(s: Seq<char>, start: int) -> bool
    recommends 0 <= start <= s.len()
{
    let (first, _) = spec_trimmed_range(s, start);
    first < s.len() && s[first] == '|'
}

/// A uniform Setext underline of at least three markers.
pub open spec fn spec_setext_underline(s: Seq<char>, start: int) -> bool
    recommends 0 <= start <= s.len()
{
    let (first, end) = spec_trimmed_range(s, start);
    first < s.len()
        && (s[first] == '=' || s[first] == '-')
        && end - first >= 3
        && spec_all_equal(s, first, end, s[first])
}

/// Count one marker in each scalar position of a bounded suffix.
pub open spec fn spec_marker_count(s: Seq<char>, start: int, end: int, marker: char) -> int
    recommends 0 <= start <= end <= s.len()
    decreases end - start
{
    if start >= end {
        0
    } else {
        (if s[start] == marker { 1int } else { 0int })
            + spec_marker_count(s, start + 1, end, marker)
    }
}

/// Check the remaining thematic-break scalars without quantifier ambiguity.
pub open spec fn spec_thematic_valid(s: Seq<char>, start: int, end: int, marker: char) -> bool
    recommends 0 <= start <= end <= s.len()
    decreases end - start
{
    if start >= end {
        true
    } else {
        (s[start] == marker || spec_is_markdown_whitespace(s[start]))
            && spec_thematic_valid(s, start + 1, end, marker)
    }
}

/// Recognize a thematic marker run, permitting only spaces and tabs between marks.
pub open spec fn spec_thematic_break(s: Seq<char>, start: int) -> bool
    recommends 0 <= start <= s.len()
{
    let (first, end) = spec_trimmed_range(s, start);
    first < s.len()
        && (s[first] == '*' || s[first] == '-' || s[first] == '_')
        && spec_thematic_valid(s, first, end, s[first])
        && spec_marker_count(s, first, end, s[first]) >= 3
}

/// ASCII digits accepted in Markdown ordered-list markers.
pub open spec fn spec_ascii_digit(ch: char) -> bool {
    matches!(ch, '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9')
}

/// Remaining ordered-list marker after the first digit.
pub open spec fn spec_ordered_list_item(
    s: Seq<char>, cursor: int, end: int, digit_count: int,
) -> bool
    recommends 0 <= cursor <= s.len(), 0 <= end <= s.len()
    decreases end - cursor
{
    if cursor >= end {
        false
    } else if s[cursor] == '.' || s[cursor] == ')' {
        cursor + 1 < s.len() && spec_is_markdown_whitespace(s[cursor + 1])
    } else if spec_ascii_digit(s[cursor]) && digit_count < 9 {
        spec_ordered_list_item(s, cursor + 1, end, digit_count + 1)
    } else {
        false
    }
}

/// Unordered or bounded ordered list marker followed by a separator.
pub open spec fn spec_list_item(s: Seq<char>, start: int) -> bool
    recommends 0 <= start <= s.len()
{
    let (first, end) = spec_trimmed_range(s, start);
    if first >= s.len() {
        false
    } else if s[first] == '-' || s[first] == '*' || s[first] == '+' {
        first + 1 < s.len() && spec_is_markdown_whitespace(s[first + 1])
    } else if spec_ascii_digit(s[first]) {
        spec_ordered_list_item(s, first + 1, end, 1)
    } else {
        false
    }
}

/// Bounded indentation width and cursor at a tab stop.
pub open spec fn spec_indentation_at(
    s: Seq<char>, cursor: int, column: int, width: int,
) -> (int, int)
    decreases s.len() - cursor
{
    if cursor >= s.len() || width >= 4 {
        (if width >= 4 { 4 } else { width }, cursor)
    } else if s[cursor] == ' ' {
        spec_indentation_at(s, cursor + 1, column, width + 1)
    } else if s[cursor] == '\t' {
        spec_indentation_at(s, cursor + 1, column, width + 4 - ((column + width) % 4))
    } else {
        (width, cursor)
    }
}

/// Scan blockquote prefixes after the outer indentation has been measured.
pub open spec fn spec_line_parts_from(
    s: Seq<char>, cursor: int, column: int, fuel: nat,
) -> (int, bool)
    decreases fuel
{
    if fuel == 0 || cursor >= s.len() {
        (cursor, false)
    } else {
        let (indent_width, after_indent) = spec_indentation_at(s, cursor, column, 0);
        if indent_width >= 4 || after_indent >= s.len() || s[after_indent] != '>' {
            (cursor, indent_width >= 4)
        } else {
            let next = after_indent + 1;
            let next_column = (column + indent_width + 1) % 4;
            if next < s.len() && s[next] == ' ' {
                spec_line_parts_from(s, next + 1, (next_column + 1) % 4, (fuel - 1) as nat)
            } else {
                spec_line_parts_from(s, next, next_column, (fuel - 1) as nat)
            }
        }
    }
}

/// Whether the current scalar cursor begins another eligible blockquote marker.
pub open spec fn spec_has_quote_prefix(s: Seq<char>, cursor: int, column: int) -> bool {
    let (indent_width, after_indent) = spec_indentation_at(s, cursor, column, 0);
    indent_width < 4 && after_indent < s.len() && s[after_indent] == '>'
}

/// Scalar body start and literal-indentation decision for the full line.
pub open spec fn spec_line_parts(s: Seq<char>) -> (int, bool) {
    let (outer_width, cursor) = spec_indentation_at(s, 0, 0, 0);
    if outer_width >= 4 {
        (0, true)
    } else {
        spec_line_parts_from(s, cursor, outer_width, (s.len() - cursor) as nat)
    }
}

} // verus!
