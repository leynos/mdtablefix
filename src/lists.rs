//! Ordered list renumbering utilities.

use std::collections::HashMap;

use regex::Regex;
use tracing::debug;

use crate::{breaks::THEMATIC_BREAK_RE, wrap::FenceTracker};

/// Characters that mark formatted text at the start of a line.
const FORMATTING_CHARS: [char; 3] = ['*', '_', '`'];

// Lines starting with optional indentation followed by '#' characters denote
// Markdown ATX headings. A space or end of line must follow the hashes.
static HEADING_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^[ ]{0,3}#{1,6}(?:\s|$)",
    "ATX heading prefix pattern should compile",
);

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

/// Remove counters for indents deeper than the given level.
/// When `inclusive` is true, levels equal to `indent` are also removed.
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

fn indent_len(indent: &str) -> usize {
    indent
        .chars()
        .fold(0, |acc, ch| acc + if ch == '\t' { 4 } else { 1 })
}

fn is_plain_paragraph_line(line: &str) -> bool {
    matches!(
        line.trim_start()
            .trim_start_matches(|c: char| FORMATTING_CHARS.contains(&c))
            .chars()
            .next(),
        Some(c) if c.is_alphanumeric()
    )
}

#[derive(Default)]
struct ListState {
    indent_stack: Vec<usize>,
    counters: HashMap<usize, usize>,
}

impl ListState {
    fn reset(&mut self) {
        debug!(
            indent_depths = self.indent_stack.len(),
            counters = self.counters.len(),
            "resetting ordered list renumbering state"
        );
        self.indent_stack.clear();
        self.counters.clear();
    }

    fn prune_deeper(&mut self, indent: usize, inclusive: bool) {
        prune_deeper(
            indent,
            inclusive,
            &mut self.indent_stack,
            &mut self.counters,
        );
    }

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
/// - Reset numbering on headings and thematic breaks.
/// - Restart numbering after a blank line followed by a plain paragraph at the same or a shallower
///   indent.
#[must_use]
pub fn renumber_lists(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut state = ListState::default();
    // Track fenced code blocks consistently across list processing.
    let mut fences = FenceTracker::default();
    #[allow(clippy::unnecessary_map_or)]
    let mut prev_blank = lines.first().map_or(true, |l| l.trim().is_empty());

    for line in lines {
        if fences.observe_line(line) {
            out.push(line.clone());
            prev_blank = false;
            continue;
        }
        if fences.in_fence_for_line(line) {
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
        if HEADING_RE.is_match(line) || THEMATIC_BREAK_RE.is_match(line.trim_end()) {
            state.reset();
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
mod tests {
    //! Unit tests for ordered list renumbering.
    //!
    //! These tests cover the parent module's parsing helpers, state
    //! transitions, and public renumbering behaviour.

    use super::*;

    #[test]
    fn parse_numbered_parts() {
        let line = "  12. item";
        assert_eq!(parse_numbered(line), Some((2, "  ", " ", "item")));
    }

    #[test]
    fn parse_numbered_with_tab() {
        let line = "	1.	foo";
        assert_eq!(parse_numbered(line), Some((4, "	", "	", "foo")));
    }

    #[test]
    fn simple_renumber() {
        let input = vec!["1. a", "3. b"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let expected = vec!["1. a", "2. b"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(renumber_lists(&input), expected);
    }

    #[test]
    fn nested_renumber() {
        let input = vec!["1. a", "    1. sub", "    3. sub2", "2. b"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let expected = vec!["1. a", "    1. sub", "    2. sub2", "2. b"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(renumber_lists(&input), expected);
    }

    #[test]
    fn list_state_reset_clears_indent_stack_and_counters() {
        let mut state = ListState::default();
        let _ = state.next_number(0);
        let _ = state.next_number(0);
        let _ = state.next_number(4);
        assert!(!state.indent_stack.is_empty());
        assert!(!state.counters.is_empty());

        state.reset();

        assert!(state.indent_stack.is_empty());
        assert!(state.counters.is_empty());
    }

    #[test]
    fn list_state_next_number_increments_and_prunes_deeper_indents() {
        let mut state = ListState::default();
        assert_eq!(state.next_number(0), 1);
        assert_eq!(state.next_number(0), 2);
        // A deeper indent starts its own counter at 1.
        assert_eq!(state.next_number(4), 1);
        assert_eq!(state.next_number(4), 2);
        // Returning to the original indent prunes the deeper one and continues
        // counting from where the outer level left off.
        assert_eq!(state.next_number(0), 3);
        assert!(!state.counters.contains_key(&4));
    }

    mod proptest_tests {
        //! Property tests for ordered list state invariants.
        //!
        //! These generated cases exercise the same `ListState` state machine
        //! used by `renumber_lists` across varied indent sequences.

        use proptest::prelude::*;

        use super::ListState;

        proptest! {
            #[test]
            fn list_state_next_number_always_starts_at_1_for_new_indent(
                indents in proptest::collection::vec(0usize..=8, 1..=20),
            ) {
                let mut state = ListState::default();
                for &indent in &indents {
                    // Capture absence before the call: `next_number` may
                    // prune deeper counters, but the counter for `indent`
                    // itself is only removed by an earlier shallower call.
                    let was_absent = !state.counters.contains_key(&indent);
                    let returned = state.next_number(indent);
                    if was_absent {
                        prop_assert_eq!(
                            returned,
                            1,
                            "indent {} first appeared (or re-emerged after pruning) but returned {}",
                            indent,
                            returned,
                        );
                    }
                }
            }

            #[test]
            fn list_state_prunes_deeper_counters_when_returning_to_outer_indent(
                outer_count in 1usize..=6,
                deeper_count in 1usize..=6,
            ) {
                let mut state = ListState::default();
                for expected in 1..=outer_count {
                    prop_assert_eq!(state.next_number(0), expected);
                }
                for expected in 1..=deeper_count {
                    prop_assert_eq!(state.next_number(4), expected);
                }

                prop_assert_eq!(state.next_number(0), outer_count + 1);
                prop_assert!(!state.counters.contains_key(&4));
                prop_assert_eq!(state.counters.get(&0), Some(&(outer_count + 2)));
            }
        }
    }
}
