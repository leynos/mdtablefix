//! Character-range predicates used by the structural scanner kernel.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

use super::OpenFence;

verified_kernel_function! {
/// Reads one scalar without exposing unchecked indexing.
pub(super) fn char_at(chars: &[char], index: usize) -> Option<char>;
ensures(result => result == if index < chars@.len() { Some(chars@[index as int]) } else { None });
{
    if index < chars.len() {
        Some(chars[index])
    } else {
        None
    }
}
}

verified_kernel_function! {
/// Reports whether every scalar is whitespace.
pub(super) fn is_blank(chars: &[char]) -> bool;
ensures(result => result == crate::spec_is_blank_from(chars@, 0));
{ is_blank_from(chars, 0) }
}

verified_loop_function! {
/// Reports whether the remaining scalars are Markdown indentation whitespace.
pub(super) fn is_blank_from(chars: &[char], start: usize) -> bool;
ensures(result => result == crate::spec_is_blank_from(chars@, start as int));
before {
    let mut cursor = start;
}
while (cursor < chars.len()) invariant(
    start <= cursor,
    cursor <= chars@.len() || cursor == start,
    forall|i: int| start <= i < cursor ==> crate::spec_is_markdown_whitespace(chars@[i]),
) {
        if !is_markdown_whitespace(chars[cursor]) {
            return false;
        }
        cursor += 1;
}
after {
    true
}
}

verified_kernel_function! {
/// Finds the range after leading and trailing whitespace.
pub(super) fn trimmed_range(chars: &[char], start: usize) -> (usize, usize);
requires(start <= chars@.len());
ensures(result =>
    start <= result.0 <= result.1 <= chars@.len(),
    result.0 == crate::spec_trim_start(chars@, start as int, chars@.len() as int),
    result.1 == crate::spec_trim_end(chars@, result.0 as int, chars@.len() as int),
);
{ trim_range(chars, start, chars.len()) }
}

verified_kernel_function! {
/// Trims whitespace inside an already-bounded scalar range.
pub(super) fn trim_range(chars: &[char], start: usize, end: usize) -> (usize, usize);
requires(start <= end, end <= chars@.len());
ensures(result =>
    start <= result.0 <= result.1 <= end,
    result.0 == crate::spec_trim_start(chars@, start as int, end as int),
    result.1 == crate::spec_trim_end(chars@, result.0 as int, end as int),
);
{
    let first = trim_start(chars, start, end);
    let last = trim_end(chars, first, end);
    (first, last)
}
}

verified_loop_function! {
/// Skips whitespace at the beginning of a bounded scalar range.
fn trim_start(chars: &[char], start: usize, end: usize) -> usize;
requires(start <= end, end <= chars@.len());
ensures(result =>
    start <= result <= end,
    result == crate::spec_trim_start(chars@, start as int, end as int),
);
before { let mut first = start; }
while (first < end && is_markdown_whitespace(chars[first])) invariant(
    start <= first <= end,
    end <= chars@.len(),
    crate::spec_trim_start(chars@, first as int, end as int)
        == crate::spec_trim_start(chars@, start as int, end as int),
) {
    first += 1;
}
after { first }
}

verified_loop_function! {
/// Skips whitespace at the end of a bounded scalar range.
fn trim_end(chars: &[char], start: usize, end: usize) -> usize;
requires(start <= end, end <= chars@.len());
ensures(result =>
    start <= result <= end,
    result == crate::spec_trim_end(chars@, start as int, end as int),
);
before { let mut last = end; }
while (last > start && is_markdown_whitespace(chars[last - 1])) invariant(
    start <= last <= end,
    end <= chars@.len(),
    crate::spec_trim_end(chars@, start as int, last as int)
        == crate::spec_trim_end(chars@, start as int, end as int),
) {
    last -= 1;
}
after { last }
}

verified_kernel_function! {
/// Reports whether a trimmed body starts with a three-character fence.
pub(super) fn is_fence_marker(chars: &[char], start: usize) -> bool;
requires(start <= chars@.len());
ensures(result => result == crate::spec_fence_marker(chars@, start as int));
{
    let (first, end) = trimmed_range(chars, start);
    match char_at(chars, first) {
        Some(marker @ ('`' | '~')) => marker_run_len(chars, first, end, marker) >= 3,
        _ => false,
    }
}
}

verified_kernel_function! {
/// Reports whether a trimmed body is a compatible closing fence.
pub(super) fn is_closing_fence(chars: &[char], start: usize, open: OpenFence) -> bool;
requires(start <= chars@.len());
ensures(result => result == crate::spec_closing_fence(chars@, start as int, open));
{
    let (first, end) = trimmed_range(chars, start);
    let markers = marker_run_len(chars, first, end, open.marker);
    markers >= open.marker_len && markers == end - first
}
}

verified_kernel_function! {
/// Reports whether a trimmed body starts with an ATX marker and separator.
pub(super) fn is_atx_heading(chars: &[char], start: usize) -> bool;
requires(start <= chars@.len());
ensures(result => result == crate::spec_atx_heading(chars@, start as int));
{
    let (first, end) = trimmed_range(chars, start);
    let hashes = marker_run_len(chars, first, end, '#');
    hashes > 0
        && hashes <= 6
        && (first + hashes == end || is_markdown_whitespace(chars[first + hashes]))
}
}

/// Reports whether every pipe-separated cell has table delimiter grammar.
#[cfg_attr(verus_keep_ghost, verifier::external_body)]
pub(super) fn is_table_delimiter(chars: &[char], start: usize) -> bool {
    let (mut first, mut end) = trimmed_range(chars, start);
    if !contains(chars, first, end, '|') {
        return false;
    }
    while first < end && chars[first] == '|' {
        first += 1;
    }
    if end > first && chars[end - 1] == '|' {
        end -= 1;
    }
    first < end && table_cells_are_delimiters(chars, first, end)
}

verified_kernel_function! {
/// Reports whether a body starts with a pipe after whitespace.
pub(super) fn body_starts_with_pipe(chars: &[char], start: usize) -> bool;
requires(start <= chars@.len());
ensures(result => result == crate::spec_body_starts_with_pipe(chars@, start as int));
{
    matches!(char_at(chars, trimmed_range(chars, start).0), Some('|'))
}
}

verified_kernel_function! {
/// Reports whether a body is one uniform Setext marker run.
pub(super) fn is_setext_underline(chars: &[char], start: usize) -> bool;
requires(start <= chars@.len());
ensures(result => result == crate::spec_setext_underline(chars@, start as int));
{
    let (first, end) = trimmed_range(chars, start);
    match char_at(chars, first) {
        Some(marker @ ('=' | '-')) => end - first >= 3 && all_equal(chars, first, end, marker),
        _ => false,
    }
}
}

verified_kernel_function! {
/// Reports whether a body is a thematic-break marker run.
#[cfg_attr(verus_keep_ghost, verifier::external_body)]
pub(super) fn is_thematic_break(chars: &[char], start: usize) -> bool;
requires(start <= chars@.len());
ensures(result => result == crate::spec_thematic_break(chars@, start as int));
{
    let (first, end) = trimmed_range(chars, start);
    let Some(marker @ ('*' | '-' | '_')) = char_at(chars, first) else {
        return false;
    };
    let mut count: usize = 0;
    let mut cursor = first;
    while cursor < end {
        let character = chars[cursor];
        if character == marker {
            count += 1;
        } else if character != ' ' && character != '\t' {
            return false;
        }
        cursor += 1;
    }
    count >= 3
}
}

verified_kernel_function! {
/// Reports whether a body begins an ordered or unordered list item.
pub(super) fn is_list_item(chars: &[char], start: usize) -> bool;
requires(start <= chars@.len());
ensures(result => result == crate::spec_list_item(chars@, start as int));
{
    let (first, end) = trimmed_range(chars, start);
    if first == chars.len() {
        return false;
    }
    match chars[first] {
        '-' | '*' | '+' => matches!(char_at(chars, first + 1), Some(' ' | '\t')),
        '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => {
            ordered_list_item(chars, first + 1, end)
        }
        _ => false,
    }
}
}

verified_loop_function! {
/// Counts an uninterrupted marker run in a scalar range.
fn marker_run_len(chars: &[char], start: usize, end: usize, marker: char) -> usize;
requires(start <= end, end <= chars@.len());
ensures(result =>
    result <= end - start,
    result == crate::spec_marker_run_len(chars@, start as int, end as int, marker),
    forall|i: int| start <= i < start + result ==> chars@[i] == marker,
    start + result == end || chars@[start + result] != marker,
);
before {
    let mut cursor = start;
}
while (cursor < end && chars[cursor] == marker) invariant(
    start <= cursor <= end,
    end <= chars@.len(),
    cursor - start + crate::spec_marker_run_len(chars@, cursor as int, end as int, marker)
        == crate::spec_marker_run_len(chars@, start as int, end as int, marker),
    forall|i: int| start <= i < cursor ==> chars@[i] == marker,
) {
        cursor += 1;
}
after {
    cursor - start
}
}

verified_kernel_function! {
/// Reports whether a scalar is Markdown's space or tab indentation whitespace.
fn is_markdown_whitespace(character: char) -> bool;
ensures(result => result == crate::spec_is_markdown_whitespace(character));
{ matches!(character, ' ' | '\t') }
}

verified_loop_function! {
/// Reports whether a scalar range contains the target character.
fn contains(chars: &[char], start: usize, end: usize, target: char) -> bool;
requires(start <= end, end <= chars@.len());
ensures(result => result == crate::spec_contains(chars@, start as int, end as int, target));
before {
    let mut cursor = start;
}
while (cursor < end) invariant(
    start <= cursor <= end,
    end <= chars@.len(),
    forall|i: int| start <= i < cursor ==> chars@[i] != target,
) {
        if chars[cursor] == target {
            return true;
        }
        cursor += 1;
}
after {
    false
}
}

verified_loop_function! {
/// Reports whether every scalar in a range equals the expected character.
fn all_equal(chars: &[char], start: usize, end: usize, expected: char) -> bool;
requires(start <= end, end <= chars@.len());
ensures(result => result == crate::spec_all_equal(chars@, start as int, end as int, expected));
before {
    let mut cursor = start;
}
while (cursor < end) invariant(
    start <= cursor <= end,
    end <= chars@.len(),
    forall|i: int| start <= i < cursor ==> chars@[i] == expected,
) {
        if chars[cursor] != expected {
            return false;
        }
        cursor += 1;
}
after {
    true
}
}

/// Reports whether each cell in a pipe-separated range is a delimiter cell.
#[cfg_attr(verus_keep_ghost, verifier::external_body)]
fn table_cells_are_delimiters(chars: &[char], start: usize, end: usize) -> bool {
    let mut cell_start = start;
    let mut cursor = start;
    while cursor <= end {
        if cursor == end || chars[cursor] == '|' {
            if !is_table_delimiter_cell(chars, cell_start, cursor) {
                return false;
            }
            cell_start = cursor + 1;
        }
        cursor += 1;
    }
    true
}

/// Reports whether one bounded table cell contains a valid alignment marker.
#[cfg_attr(verus_keep_ghost, verifier::external_body)]
fn is_table_delimiter_cell(chars: &[char], start: usize, end: usize) -> bool {
    let (mut first, mut last) = trim_range(chars, start, end);
    if first < last && chars[first] == ':' {
        first += 1;
    }
    if first < last && chars[last - 1] == ':' {
        last -= 1;
    }
    first < last && all_equal(chars, first, last, '-')
}

verified_kernel_function! {
/// Reports whether a digit run closes with ordered-list punctuation and space.
#[cfg_attr(verus_keep_ghost, verifier::external_body)]
fn ordered_list_item(chars: &[char], start: usize, end: usize) -> bool;
requires(start <= chars@.len(), end <= chars@.len());
ensures(result => result == crate::spec_ordered_list_item(chars@, start as int, end as int, 1));
{
    let mut digit_count = 1;
    let mut cursor = start;
    while cursor < end {
        match chars[cursor] {
            '.' | ')' => return matches!(char_at(chars, cursor + 1), Some(' ' | '\t')),
            '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' if digit_count < 9 => {
                digit_count += 1;
                cursor += 1;
            }
            _ => return false,
        }
    }
    false
}
}
