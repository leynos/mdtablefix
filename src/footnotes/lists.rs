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
        .rfind(|&i| !predicate(lines[i].trim_end()))
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
        && lines[start..end]
            .iter()
            .any(|line| FOOTNOTE_LINE_RE.is_match(line))
    {
        Some((start, end))
    } else {
        None
    }
}

/// Determine whether a second-level heading precedes the block.
pub(super) fn has_h2_heading_before(lines: &[String], start: usize) -> bool {
    lines[..start]
        .iter()
        .rfind(|l| !l.trim().is_empty())
        .is_some_and(|l| l.trim_start().starts_with("## "))
}

/// Check for existing footnote definitions before the block.
pub(super) fn has_existing_footnote_block(lines: &[String], start: usize) -> bool {
    let mut fences = FenceTracker::default();
    for l in &lines[..start] {
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

/// One line together with the capture match taken over it.
///
/// The two are inseparable: every capture is a byte range *within* `text`, so
/// a match paired with any other line would name a plausible but wrong slice.
/// Keeping them in one value makes that pairing impossible to get wrong.
struct MatchedLine<'a> {
    /// The line the pattern matched.
    text: &'a str,
    /// The groups the pattern captured over `text`.
    caps: &'a Captures<'a>,
}

impl<'a> MatchedLine<'a> {
    /// Pairs a matched line with its captures.
    fn new(text: &'a str, caps: &'a Captures<'a>) -> Self { Self { text, caps } }

    /// Reads the separator between the item number and the item text.
    ///
    /// The separator is taken from the original bytes rather than rebuilt, so
    /// list formatting and continuation alignment stay stable. Returns `None`
    /// when the slice is not a valid boundary or a mandatory group is absent.
    fn separator_after_number(&self) -> Option<&'a str> {
        let num_match = self.caps.name("num")?;
        let rest_match = self.caps.name("rest")?;
        self.text.get(num_match.end() + 1..rest_match.start())
    }

    /// The block's indentation, as captured.
    fn indent(&self) -> Option<&'a str> { Some(self.caps.name("indent")?.as_str()) }

    /// The item's own text, as captured.
    fn rest(&self) -> Option<&'a str> { Some(self.caps.name("rest")?.as_str()) }
}

/// Rewrites one matched list item as a footnote definition header.
///
/// Returns `None` when a capture the pattern makes mandatory is absent; both
/// groups are non-optional in `FOOTNOTE_LINE_RE`, so that cannot arise from a
/// successful match, and the caller leaves the matched text unchanged rather
/// than inventing a replacement.
fn rewrite_footnote_line(matched: &MatchedLine<'_>, number: usize) -> Option<String> {
    let indent = matched.indent()?;
    let whitespace = matched.separator_after_number()?;
    let rest = matched.rest()?;
    Some(format!("{indent}[^{number}]:{whitespace}{rest}"))
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
            let matched = caps.get(0).map_or("", |whole| whole.as_str());
            let matched_line = MatchedLine::new(line, caps);
            rewrite_footnote_line(&matched_line, number).unwrap_or_else(|| matched.to_string())
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
    for line in &mut lines[start..end] {
        if FOOTNOTE_LINE_RE.is_match(line) {
            *line = replace_footnote_line(line, number);
            number += 1;
        }
    }
}
