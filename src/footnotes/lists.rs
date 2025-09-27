//! Trailing list conversion for footnotes.
//!
//! Identifies eligible trailing ordered lists and rewrites them into
//! definition blocks when the surrounding context allows.

use regex::Captures;

use super::parsing::{FOOTNOTE_LINE_RE, is_definition_continuation};

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
    let mut in_fence = false;
    for l in &lines[..start] {
        let t = l.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut t = t;
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

fn replace_footnote_line(line: &str) -> String {
    FOOTNOTE_LINE_RE
        .replace(line, |caps: &Captures| {
            let num_match = caps
                .name("num")
                .expect("footnote line capture missing number");
            let rest_match = caps
                .name("rest")
                .expect("footnote line capture missing rest");
            let whitespace = &line[num_match.end() + 1..rest_match.start()];
            format!(
                "{}[^{}]:{}{}",
                &caps["indent"], &caps["num"], whitespace, &caps["rest"]
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
    for line in &mut lines[start..end] {
        if FOOTNOTE_LINE_RE.is_match(line) {
            *line = replace_footnote_line(line);
        }
    }
}
