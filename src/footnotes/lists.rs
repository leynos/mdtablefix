//! Trailing list conversion for footnotes.
//!
//! Identifies eligible trailing ordered lists and rewrites them into
//! definition blocks when the surrounding context allows.

use regex::Captures;

use super::parsing::{FOOTNOTE_LINE_RE, is_definition_continuation};
use crate::wrap::FenceTracker;

/// Find the trailing block of lines that satisfy a predicate.
pub(super) fn trimmed_range<F>(lines: &[String], predicate: F) -> (usize, usize)
where
    F: Fn(&str) -> bool,
{
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |i| i + 1);
    let start = (0..end)
        .rfind(|&i| lines.get(i).is_some_and(|line| !predicate(line.trim_end())))
        .map_or(0, |i| i + 1);
    (start, end)
}

/// Identify the trailing block of blank or footnote-like lines.
pub(super) fn footnote_block_range(lines: &[String]) -> Option<(usize, usize)> {
    let (start, end) = trimmed_range(lines, |line| {
        line.trim().is_empty()
            || FOOTNOTE_LINE_RE.is_match(line)
            || is_definition_continuation(line)
    });
    if start < end
        && lines
            .get(start..end)
            .is_some_and(|block| block.iter().any(|line| FOOTNOTE_LINE_RE.is_match(line)))
    {
        Some((start, end))
    } else {
        None
    }
}

/// Determine whether a second-level heading precedes the block.
pub(super) fn has_h2_heading_before(lines: &[String], start: usize) -> bool {
    lines
        .get(..start)
        .unwrap_or_default()
        .iter()
        .rfind(|l| !l.trim().is_empty())
        .is_some_and(|l| l.trim_start().starts_with("## "))
}

/// Check for existing footnote definitions before the block.
pub(super) fn has_existing_footnote_block(lines: &[String], start: usize) -> bool {
    let mut fences = FenceTracker::default();
    for l in lines.get(..start).unwrap_or_default() {
        let fence = fences.observe_source_line(l);
        if fence.is_fence_marker || fence.is_in_fence {
            continue;
        }
        let mut t = l.trim_start();
        while let Some(rest) = t.strip_prefix('>') {
            t = rest.trim_start();
        }
        if t.strip_prefix("[^")
            .and_then(|r| r.split_once("]:"))
            .is_some_and(|(num, _)| num.chars().all(|c| c.is_ascii_digit()))
        {
            return true;
        }
    }
    false
}

/// Rewrites an ordered-list item as a footnote definition header.
///
/// `number` is the number the fold assigns rather than the item's own, because
/// the two differ whenever the list is not already numbered from one — `10.`
/// is the third item of a list of three. The fold reaches that list only when
/// nothing has claimed a number yet: the label stage promotes every item of
/// such a list in its own scan as soon as any reference or definition exists,
/// so a list still numbered here belongs to a document with neither, and the
/// numbers it takes start at one and run in list order.
///
/// The capture boundaries are used instead of trimming so list formatting and
/// continuation alignment remain stable after conversion.
fn replace_footnote_line(line: &str, number: usize) -> String {
    FOOTNOTE_LINE_RE
        .replace(line, |caps: &Captures| {
            let num_match = caps
                .name("num")
                .expect("footnote line capture missing number");
            let rest_match = caps
                .name("rest")
                .expect("footnote line capture missing rest");
            let whitespace = line
                .get(num_match.end() + 1..rest_match.start())
                .unwrap_or("");
            format!(
                "{}[^{number}]:{}{}",
                caps.name("indent").map_or("", |m| m.as_str()),
                whitespace,
                caps.name("rest").map_or("", |m| m.as_str())
            )
        })
        .to_string()
}

/// Convert the trailing ordered list block into footnote definitions when allowed.
pub(super) fn convert_block(lines: &mut [String]) {
    let Some((start, end)) = footnote_block_range(lines) else {
        return;
    };
    if !has_h2_heading_before(lines, start) || has_existing_footnote_block(lines, start) {
        return;
    }
    let mut number = 1;
    let Some(block) = lines.get_mut(start..end) else {
        return;
    };
    for line in block {
        if FOOTNOTE_LINE_RE.is_match(line) {
            *line = replace_footnote_line(line, number);
            number += 1;
        }
    }
}
