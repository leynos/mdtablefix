//! Ordered list renumbering utilities.

use std::collections::HashMap;

use regex::Regex;
use tracing::debug;

use crate::{
    classify::{ClassifyCtx, LineClass, classify_line_with_body, list_content_indent},
    wrap::FenceTracker,
};

/// Characters that mark formatted text at the start of a line.
const FORMATTING_CHARS: [char; 3] = ['*', '_', '`'];

/// Splits a numbered list item into indentation, separator, and content slices.
///
/// The returned indentation width treats a tab as four columns so nested counters use the same
/// depth model as the Markdown block parser.
fn parse_numbered(line: &str) -> Option<(usize, &str, &str, &str)> {
    static NUMBERED_RE: std::sync::LazyLock<Regex> = lazy_regex!(
        r"^(\s*)(?:[1-9][0-9]*)\.(\s+)(.*)",
        "numbered list item pattern should compile",
    );
    let cap = NUMBERED_RE.captures(line)?;
    let indent_str = cap.get(1)?.as_str();
    let indent = indent_len(indent_str);
    let sep = cap.get(2)?.as_str();
    let rest = cap.get(3)?.as_str();
    Some((indent, indent_str, sep, rest))
}

/// Returns the column where a renumbered item's content starts, in parser columns.
///
/// Measured on the emitted marker, `number` and its dot, because the
/// emitted line is what a second pass reads: measuring the source marker
/// would let `10.` becoming `2.` move a block into the item between passes.
/// The separator follows `classify::list_content_indent`, tab stops
/// included, so both passes agree on where an item's content starts.
fn content_column(indent: usize, number: usize, sep: &str, rest: &str) -> usize {
    list_content_indent(&format!("{number}.{sep}{rest}"), indent)
}

/// Rebuilds a line with `strip` columns of its indentation removed.
///
/// Classifying a line relative to the item that contains it lets a block
/// indented four or more columns in absolute terms read as the heading or
/// break it is inside that item, rather than as indented code.
fn relative_to(line: &str, indent: usize, strip: usize) -> String {
    format!(
        "{}{}",
        " ".repeat(indent.saturating_sub(strip)),
        line.trim_start()
    )
}

/// Removes counters deeper than the current list item, optionally including its own depth.
fn prune_deeper(
    indent: usize,
    inclusive: bool,
    indent_stack: &mut Vec<usize>,
    counters: &mut HashMap<usize, usize>,
) {
    while indent_stack
        .last()
        .is_some_and(|&d| if inclusive { d >= indent } else { d > indent })
    {
        if let Some(d) = indent_stack.pop() {
            counters.remove(&d);
        }
    }
}

/// Measures indentation in parser columns, expanding tabs to four columns.
fn indent_len(indent: &str) -> usize {
    indent
        .chars()
        .fold(0, |acc, ch| acc + if ch == '\t' { 4 } else { 1 })
}

/// Reports whether a non-list line begins with ordinary alphanumeric paragraph text.
fn is_plain_paragraph_line(line: &str) -> bool {
    matches!(
        line.trim_start()
            .trim_start_matches(|c: char| FORMATTING_CHARS.contains(&c))
            .chars()
            .next(),
        Some(c) if c.is_alphanumeric()
    )
}

/// Holds ordered-list counters keyed by indentation depth.
#[derive(Default)]
struct ListState {
    /// Active list indentation levels ordered from outermost to innermost.
    indent_stack: Vec<usize>,
    /// Next item number for each active indentation level.
    counters: HashMap<usize, usize>,
    /// Content column of the latest item at each active indentation level.
    ///
    /// A block belongs to that item only when it is indented at least this
    /// far, which is what decides whether it ends the list.
    content_columns: HashMap<usize, usize>,
}

impl ListState {
    /// Removes nested counters before handling a new item or paragraph restart.
    fn prune_deeper(&mut self, indent: usize, inclusive: bool) {
        prune_deeper(
            indent,
            inclusive,
            &mut self.indent_stack,
            &mut self.counters,
        );
    }

    /// Allocates the next number at an indentation level, starting at one.
    fn next_number(&mut self, indent: usize) -> usize {
        self.prune_deeper(indent, false);
        if self.indent_stack.last().is_none_or(|&d| d < indent) {
            self.indent_stack.push(indent);
        }
        let num = self.counters.entry(indent).or_insert(1);
        let current = *num;
        *num += 1;
        current
    }

    /// Returns the content column of the innermost active item that contains `indent`.
    ///
    /// Zero when no active item's content column is at or left of `indent`.
    fn containing_content_column(&self, indent: usize) -> usize {
        self.indent_stack
            .iter()
            .filter_map(|depth| self.content_columns.get(depth).copied())
            .filter(|&column| column <= indent)
            .max()
            .unwrap_or(0)
    }

    /// Records where the latest item at `indent` starts its content.
    fn record_content_column(&mut self, indent: usize, column: usize) {
        self.content_columns.insert(indent, column);
    }

    /// Ends every list whose current item cannot contain a block at `column`.
    ///
    /// A block belongs to an item only when it is indented to the item's
    /// content column, so a block left of that column ends the list and the
    /// next marker at that depth starts a new one. Lists whose items do
    /// contain the block continue. A depth with no recorded content column
    /// is treated as ending right after its marker column.
    fn end_lists_at(&mut self, column: usize) {
        debug!(
            column,
            indent_depths = self.indent_stack.len(),
            "ending ordered lists whose items cannot contain the block"
        );
        while let Some(&depth) = self.indent_stack.last() {
            let content = self
                .content_columns
                .get(&depth)
                .copied()
                .unwrap_or(depth + 1);
            if content <= column {
                break;
            }
            self.indent_stack.pop();
            self.counters.remove(&depth);
            self.content_columns.remove(&depth);
        }
    }

    /// Resets the current level after a blank line followed by a plain paragraph.
    fn handle_paragraph_restart(&mut self, indent: usize, line: &str, prev_blank: bool) -> bool {
        let inclusive = prev_blank
            && self
                .indent_stack
                .last()
                .is_some_and(|&depth| indent <= depth && is_plain_paragraph_line(line));
        if inclusive {
            self.prune_deeper(indent, true);
        }
        inclusive
    }
}

/// Renumber ordered Markdown list items across the given lines.
/// - Preserve code fences; do not renumber inside them.
/// - End the lists at or right of a heading's or thematic break's column; one indented into an item
///   leaves that item's list counting, and one at column 0 ends every list.
/// - Restart numbering after a blank line followed by a plain paragraph at the same or a shallower
///   indent.
#[must_use]
pub fn renumber_lists(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut state = ListState::default();
    // Track fenced code blocks consistently across list processing.
    let mut fences = FenceTracker::default();
    let mut prev_blank = lines.first().is_none_or(|l| l.trim().is_empty());

    for line in lines {
        let fence = fences.observe_source_line(line);
        if fence.is_fence_marker {
            out.push(line.clone());
            prev_blank = false;
            continue;
        }
        if fence.is_in_fence {
            out.push(line.clone());
            prev_blank = line.trim().is_empty();
            continue;
        }
        if line.trim().is_empty() {
            out.push(line.clone());
            prev_blank = true;
            continue;
        }
        if let Some((indent, indent_str, sep, rest)) = parse_numbered(line) {
            let current = state.next_number(indent);
            state.record_content_column(indent, content_column(indent, current, sep, rest));
            out.push(format!("{indent_str}{current}.{sep}{rest}"));
            prev_blank = false;
            continue;
        }
        let indent_end = line
            .char_indices()
            .find(|&(_, c)| !c.is_whitespace())
            .map_or_else(|| line.len(), |(i, _)| i);
        let indent_str = &line[..indent_end];
        let indent = indent_len(indent_str);
        let relative = relative_to(line, indent, state.containing_content_column(indent));
        let classified = classify_line_with_body(&relative, &ClassifyCtx::default());
        let prefix = &relative[..relative.len() - classified.body.len()];
        if !prefix.contains('>')
            && matches!(
                classified.class,
                LineClass::AtxHeading | LineClass::ThematicBreak
            )
        {
            // A heading or break indented into an item is a child block of
            // that item, so only the lists at or right of its column end
            // (issue #450); at column 0 that is every list.
            state.end_lists_at(indent);
            out.push(line.clone());
            prev_blank = false;
            continue;
        }
        let did_inclusive = state.handle_paragraph_restart(indent, line, prev_blank);
        if !did_inclusive {
            state.prune_deeper(indent, false);
        }
        out.push(line.clone());
        prev_blank = false;
    }
    out
}

#[cfg(test)]
#[path = "lists_tests.rs"]
mod tests;
