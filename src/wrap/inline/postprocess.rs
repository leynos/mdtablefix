//! Post-wrap normalization helpers for inline fragment lines.
//!
//! After `textwrap::wrap_algorithms::wrap_first_fit` assigns fragments to
//! lines, two passes correct edge cases that the greedy algorithm cannot
//! anticipate:
//!
//! 1. `merge_whitespace_only_lines` absorbs separator lines that consist
//!    entirely of whitespace fragments back into adjacent content lines so
//!    that rendered output does not gain spurious blank entries.
//! 2. `rebalance_atomic_tails` moves a trailing atomic or plain fragment from
//!    one line to the beginning of the next when doing so keeps both lines
//!    within the target width, preventing orphaned punctuation or code spans.

use super::{FragmentKind, InlineFragment};

/// Returns `true` when every fragment on `line` is a whitespace fragment.
fn is_whitespace_only_line(line: &[InlineFragment]) -> bool {
    line.iter().all(InlineFragment::is_whitespace)
}

/// Returns `true` when `line` contains exactly one fragment whose text is a
/// single ASCII space character.
fn is_single_space_line(line: &[InlineFragment]) -> bool { line.len() == 1 && line[0].text == " " }

/// Returns the total display-column width of all fragments on `line`.
fn line_width(line: &[InlineFragment]) -> usize { line.iter().map(|fragment| fragment.width).sum() }

/// Returns `true` when the first fragment on `line` is an atomic fragment
/// (inline code span or Markdown link).
fn line_starts_with_atomic(line: &[InlineFragment]) -> bool {
    line.first().is_some_and(InlineFragment::is_atomic)
}

/// Returns `true` when `line` starts with a single-space whitespace fragment
/// followed by at least one plain-text fragment.
///
/// This pattern identifies lines that are candidates for receiving a
/// rebalanced tail fragment from the previous line.
fn line_starts_with_single_space_then_plain(line: &[InlineFragment]) -> bool {
    line.first()
        .is_some_and(|fragment| fragment.is_whitespace() && fragment.text == " ")
        && line.get(1).is_some_and(InlineFragment::is_plain)
}

/// Returns `true` when `line` ends with an atomic fragment, or ends with a
/// plain-text fragment that is not the only fragment on the line.
///
/// Both cases are eligible for tail rebalancing: moving the last fragment to
/// the following line when width permits.
fn line_has_rebalanceable_tail(line: &[InlineFragment]) -> bool {
    line.last().is_some_and(InlineFragment::is_atomic)
        || (line.len() > 1 && line.last().is_some_and(InlineFragment::is_plain))
}

/// Merges whitespace-only lines produced by `wrap_first_fit` back into
/// adjacent content lines.
///
/// `textwrap` may split a separator whitespace fragment onto its own line when
/// it occurs at a break boundary. Such lines would render as spurious blank
/// output entries. This function accumulates pending whitespace and prepends
/// it to the next non-whitespace line, with two special cases:
///
/// - When a single-space line follows a line whose last fragment is an inline
///   code span and the next line does not start with an atomic fragment, the
///   code span is popped back into the pending buffer so it will flow together
///   with the following content.
/// - When a single-space line follows a line that contains only one fragment,
///   the space is carried forward rather than being dropped, preserving the
///   spacing relationship between the two fragments.
///
/// Any pending whitespace that remains after all lines have been processed is
/// appended to the last merged line.
pub(super) fn merge_whitespace_only_lines(
    lines: &[Vec<InlineFragment>],
) -> Vec<Vec<InlineFragment>> {
    let mut merged: Vec<Vec<InlineFragment>> = Vec::with_capacity(lines.len());
    let mut pending_whitespace: Vec<InlineFragment> = Vec::new();

    for (index, mut line) in lines.iter().cloned().enumerate() {
        if is_whitespace_only_line(&line) {
            let next_starts_atomic = lines
                .get(index + 1)
                .is_some_and(|next_line| line_starts_with_atomic(next_line));
            let line_is_single_space = is_single_space_line(&line);
            let previous_line_has_single_fragment = merged
                .last()
                .is_some_and(|previous_line| previous_line.len() == 1);
            let mut should_carry_whitespace = !line_is_single_space;

            if line_is_single_space
                && !next_starts_atomic
                && let Some(previous_line) = merged.last_mut()
                && previous_line
                    .last()
                    .is_some_and(|fragment| fragment.kind == FragmentKind::InlineCode)
            {
                let previous_atomic = previous_line
                    .pop()
                    .expect("line with an atomic tail contains that fragment");
                pending_whitespace.push(previous_atomic);
                if previous_line.is_empty() {
                    merged.pop();
                }
                should_carry_whitespace = true;
            }

            if line_is_single_space && previous_line_has_single_fragment {
                should_carry_whitespace = true;
            }

            if should_carry_whitespace {
                pending_whitespace.extend(line);
            }
            continue;
        }

        if pending_whitespace.is_empty() {
            merged.push(line);
        } else {
            pending_whitespace.append(&mut line);
            merged.push(std::mem::take(&mut pending_whitespace));
        }
    }

    if !pending_whitespace.is_empty() {
        if let Some(last_line) = merged.last_mut() {
            last_line.append(&mut pending_whitespace);
        } else {
            merged.push(pending_whitespace);
        }
    }

    merged
}

/// Moves trailing atomic or plain fragments to the following line when doing
/// so keeps both lines within `width` display columns.
///
/// After `merge_whitespace_only_lines` normalises separator lines, a wrapped
/// line may still end with an isolated code span or word that would read more
/// naturally at the start of the next line. This function iterates over
/// adjacent line pairs and, when the next line starts with a single-space
/// separator followed by a plain fragment, moves the current line's last
/// fragment forward if the resulting next-line width remains within `width`.
///
/// The width guard ensures that this heuristic never creates a line that
/// `wrap_first_fit` would itself have rejected.
pub(super) fn rebalance_atomic_tails(lines: &mut [Vec<InlineFragment>], width: usize) {
    for index in 0..lines.len().saturating_sub(1) {
        if !line_starts_with_single_space_then_plain(&lines[index + 1])
            || !line_has_rebalanceable_tail(&lines[index])
        {
            continue;
        }

        let trailing_width = lines[index]
            .last()
            .map(|fragment| fragment.width)
            .expect("line selected for tail rebalancing contains a trailing fragment");
        if line_width(&lines[index + 1]) + trailing_width > width {
            continue;
        }

        let trailing_fragment = lines[index]
            .pop()
            .expect("line selected for tail rebalancing contains a trailing fragment");
        lines[index + 1].insert(0, trailing_fragment);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{FragmentKind, InlineFragment};
    use super::{merge_whitespace_only_lines, rebalance_atomic_tails};

    #[test]
    fn inline_fragment_new_classifies_whitespace() {
        let fragment = InlineFragment::new(" ".to_string());
        assert_eq!(fragment.kind, FragmentKind::Whitespace);
        assert_eq!(fragment.width, 1);
        assert!(fragment.is_whitespace());
        assert!(!fragment.is_atomic());
        assert!(!fragment.is_plain());
    }

    #[test]
    fn inline_fragment_new_classifies_inline_code() {
        let fragment = InlineFragment::new("`code`".to_string());
        assert_eq!(fragment.kind, FragmentKind::InlineCode);
        assert_eq!(fragment.width, 6);
        assert!(!fragment.is_whitespace());
        assert!(fragment.is_atomic());
        assert!(!fragment.is_plain());
    }

    #[test]
    fn inline_fragment_new_classifies_link() {
        let fragment = InlineFragment::new("[text](url)".to_string());
        assert_eq!(fragment.kind, FragmentKind::Link);
        assert_eq!(fragment.width, 11);
        assert!(!fragment.is_whitespace());
        assert!(fragment.is_atomic());
        assert!(!fragment.is_plain());
    }

    #[test]
    fn inline_fragment_new_classifies_plain_text() {
        let fragment = InlineFragment::new("hello".to_string());
        assert_eq!(fragment.kind, FragmentKind::Plain);
        assert_eq!(fragment.width, 5);
        assert!(!fragment.is_whitespace());
        assert!(!fragment.is_atomic());
        assert!(fragment.is_plain());
    }

    #[test]
    fn inline_fragment_new_classifies_code_with_trailing_punct() {
        let fragment = InlineFragment::new("`code`.".to_string());
        assert_eq!(fragment.kind, FragmentKind::InlineCode);
        assert!(fragment.is_atomic());
    }

    #[test]
    fn inline_fragment_new_classifies_link_with_trailing_punct() {
        let fragment = InlineFragment::new("[text](url).".to_string());
        assert_eq!(fragment.kind, FragmentKind::Link);
        assert!(fragment.is_atomic());
    }

    #[test]
    fn inline_fragment_new_computes_width_from_display_columns() {
        let fragment = InlineFragment::new("abc".to_string());
        assert_eq!(fragment.width, 3);
    }

    #[test]
    fn inline_fragment_new_handles_empty_string() {
        let fragment = InlineFragment::new(String::new());
        assert_eq!(fragment.width, 0);
        assert_eq!(fragment.kind, FragmentKind::Plain);
    }

    fn plain(text: &str) -> InlineFragment {
        InlineFragment { text: text.to_string(), width: text.len(), kind: FragmentKind::Plain }
    }

    fn ws(text: &str) -> InlineFragment {
        InlineFragment {
            text: text.to_string(),
            width: text.len(),
            kind: FragmentKind::Whitespace,
        }
    }

    fn code(text: &str) -> InlineFragment {
        InlineFragment {
            text: text.to_string(),
            width: text.len(),
            kind: FragmentKind::InlineCode,
        }
    }

    fn link(text: &str) -> InlineFragment {
        InlineFragment { text: text.to_string(), width: text.len(), kind: FragmentKind::Link }
    }

    #[test]
    fn merge_whitespace_only_lines_absorbs_whitespace_line_into_next() {
        let lines = vec![vec![plain("foo")], vec![ws(" ")], vec![plain("bar")]];
        let merged = merge_whitespace_only_lines(&lines);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], vec![plain("foo")]);
        assert_eq!(merged[1], vec![ws(" "), plain("bar")]);
    }

    #[test]
    fn merge_whitespace_only_lines_appends_trailing_whitespace_to_last_line() {
        let lines = vec![vec![plain("foo")], vec![ws("  ")]];
        let merged = merge_whitespace_only_lines(&lines);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0], vec![plain("foo"), ws("  ")]);
    }

    #[test]
    fn merge_whitespace_only_lines_pops_inline_code_before_single_space() {
        // A single-space whitespace line following a line ending with InlineCode
        // and not preceding an atomic fragment should pop the code span back
        // into pending so it flows with the next line.
        let next_plain = vec![plain("baz")];
        let lines = vec![vec![plain("foo"), code("`x`")], vec![ws(" ")], next_plain.clone()];
        let merged = merge_whitespace_only_lines(&lines);
        // The code span and space should be prepended to the next content line.
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], vec![plain("foo")]);
        assert!(merged[1].contains(&code("`x`")));
        assert!(merged[1].contains(&plain("baz")));
    }

    #[test]
    fn merge_whitespace_only_lines_preserves_non_whitespace_lines_unchanged() {
        let lines = vec![vec![plain("hello")], vec![plain("world")]];
        let merged = merge_whitespace_only_lines(&lines);
        assert_eq!(merged, lines);
    }

    #[test]
    fn rebalance_atomic_tails_moves_code_span_to_next_line_when_it_fits() {
        // Line 0: ["word", `code`]  widths 4 + 6 = 10
        // Line 1: [" ", "next"]    widths 1 + 4 = 5  -> after move: 6 + 1 + 4 = 11 > 12? No: 6+5=11 <= 12
        let mut lines = vec![
            vec![plain("word"), code("`code`")],
            vec![ws(" "), plain("next")],
        ];
        rebalance_atomic_tails(&mut lines, 12);
        // `code` should move to line 1 because 6 + 1 + 4 = 11 <= 12
        assert_eq!(lines[0], vec![plain("word")]);
        assert_eq!(lines[1][0], code("`code`"));
    }

    #[test]
    fn rebalance_atomic_tails_does_not_move_when_overflow_would_result() {
        // Line 0: ["word", `code`]  total widths for moved fragment = 6
        // Line 1: [" ", "verylongword"]  width 1 + 12 = 13; with fragment 6+13=19 > 10
        let mut lines = vec![
            vec![plain("word"), code("`code`")],
            vec![ws(" "), plain("verylongword")],
        ];
        rebalance_atomic_tails(&mut lines, 10);
        // Should not move because 6 + 1 + 12 = 19 > 10
        assert_eq!(lines[0], vec![plain("word"), code("`code`")]);
    }

    #[test]
    fn rebalance_atomic_tails_moves_link_fragment_when_fits() {
        let mut lines = vec![
            vec![plain("See"), link("[doc](u)")],
            vec![ws(" "), plain("here")],
        ];
        // [doc](u) width = 8, " here" width = 5, total = 13 <= 15
        rebalance_atomic_tails(&mut lines, 15);
        assert_eq!(lines[0], vec![plain("See")]);
        assert_eq!(lines[1][0], link("[doc](u)"));
    }
}