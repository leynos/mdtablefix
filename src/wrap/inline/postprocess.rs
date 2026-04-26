//! Post-wrap normalization helpers for inline fragment lines.

use super::{FragmentKind, InlineFragment};

/// Returns whether every fragment on the line is whitespace-only.
fn is_whitespace_only_line(line: &[InlineFragment]) -> bool {
    line.iter().all(InlineFragment::is_whitespace)
}

/// Returns whether the line consists of one literal space fragment.
fn is_single_space_line(line: &[InlineFragment]) -> bool { line.len() == 1 && line[0].text == " " }

/// Returns the total display width of the rendered line.
fn line_width(line: &[InlineFragment]) -> usize { line.iter().map(|fragment| fragment.width).sum() }

/// Returns whether the line begins with an atomic fragment.
fn line_starts_with_atomic(line: &[InlineFragment]) -> bool {
    line.first().is_some_and(InlineFragment::is_atomic)
}

/// Returns whether the line begins with a single-space carry and plain prose.
fn line_starts_with_single_space_then_plain(line: &[InlineFragment]) -> bool {
    line.first()
        .is_some_and(|fragment| fragment.is_whitespace() && fragment.text == " ")
        && line.get(1).is_some_and(InlineFragment::is_plain)
}

/// Returns whether the line ends with a fragment worth rebalancing.
fn line_has_rebalanceable_tail(line: &[InlineFragment]) -> bool {
    line.last().is_some_and(InlineFragment::is_atomic)
        || (line.len() > 1 && line.last().is_some_and(InlineFragment::is_plain))
}

/// Moves a previous inline-code tail into pending whitespace handling.
fn carry_previous_inline_code_tail(
    merged: &mut Vec<Vec<InlineFragment>>,
    pending_whitespace: &mut Vec<InlineFragment>,
) -> bool {
    let Some(previous_line) = merged.last_mut() else {
        return false;
    };
    if !previous_line
        .last()
        .is_some_and(|fragment| fragment.kind == FragmentKind::InlineCode)
    {
        return false;
    }

    let Some(previous_atomic) = previous_line.pop() else {
        return false;
    };
    pending_whitespace.push(previous_atomic);
    if previous_line.is_empty() {
        merged.pop();
    }
    true
}

/// Merges whitespace-only wrap artefacts into neighbouring content lines.
///
/// `lines` is the provisional fragment layout from `wrap_first_fit`, and the
/// return value folds whitespace-only lines into adjacent content. A single
/// space after an inline-code tail is carried forward with that atomic
/// fragment instead of being merged backward. This helper never panics.
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
                && carry_previous_inline_code_tail(&mut merged, &mut pending_whitespace)
            {
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

/// Moves an eligible tail fragment onto the following line when it still fits.
///
/// `lines` is mutated in place after whitespace-line merging, and `width` is
/// the maximum display width allowed for the destination line. The move is
/// applied only when the next line starts with a carried single space plus
/// plain text and the new width stays within `width`. This helper never
/// panics.
pub(super) fn rebalance_atomic_tails(lines: &mut [Vec<InlineFragment>], width: usize) {
    for index in 0..lines.len().saturating_sub(1) {
        if !line_starts_with_single_space_then_plain(&lines[index + 1])
            || !line_has_rebalanceable_tail(&lines[index])
        {
            continue;
        }

        let Some(trailing_width) = lines[index].last().map(|fragment| fragment.width) else {
            continue;
        };
        if line_width(&lines[index + 1]) + trailing_width > width {
            continue;
        }

        let Some(trailing_fragment) = lines[index].pop() else {
            continue;
        };
        lines[index + 1].insert(0, trailing_fragment);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::{FragmentKind, InlineFragment},
        *,
    };

    fn fragment(text: &str) -> InlineFragment { InlineFragment::new(text.into()) }

    #[test]
    fn inline_fragment_whitespace_space() {
        let fragment = InlineFragment::new(" ".into());
        assert_eq!(fragment.kind, FragmentKind::Whitespace);
        assert!(fragment.is_whitespace());
        assert!(!fragment.is_atomic());
        assert_eq!(fragment.width, 1);
    }

    #[test]
    fn inline_fragment_whitespace_tab() {
        let fragment = InlineFragment::new("\t".into());
        assert_eq!(fragment.kind, FragmentKind::Whitespace);
    }

    #[test]
    fn inline_fragment_inline_code() {
        let fragment = InlineFragment::new("`foo`".into());
        assert_eq!(fragment.kind, FragmentKind::InlineCode);
        assert!(fragment.is_atomic());
        assert!(!fragment.is_whitespace());
        assert!(!fragment.is_plain());
    }

    #[test]
    fn inline_fragment_link() {
        let fragment = InlineFragment::new("[text](url)".into());
        assert_eq!(fragment.kind, FragmentKind::Link);
        assert!(fragment.is_atomic());
    }

    #[test]
    fn inline_fragment_plain() {
        let fragment = InlineFragment::new("word".into());
        assert_eq!(fragment.kind, FragmentKind::Plain);
        assert!(fragment.is_plain());
        assert!(!fragment.is_atomic());
    }

    #[test]
    fn merge_keeps_content_lines_unchanged() {
        let lines = vec![vec![fragment("hello")], vec![fragment("world")]];
        assert_eq!(merge_whitespace_only_lines(&lines), lines);
    }

    #[test]
    fn merge_carries_whitespace_forward() {
        let lines = vec![
            vec![fragment("hello")],
            vec![fragment(" ")],
            vec![fragment("world")],
        ];
        assert_eq!(
            merge_whitespace_only_lines(&lines),
            vec![
                vec![fragment("hello")],
                vec![fragment(" "), fragment("world")],
            ]
        );
    }

    #[test]
    fn merge_moves_inline_code_tail_before_single_space() {
        let lines = vec![
            vec![fragment("plain"), fragment("`code`")],
            vec![fragment(" ")],
            vec![fragment("tail")],
        ];
        let merged = merge_whitespace_only_lines(&lines);

        assert_eq!(merged[0], vec![fragment("plain")]);
        assert_eq!(
            merged[1],
            vec![fragment("`code`"), fragment(" "), fragment("tail")]
        );
    }

    #[test]
    fn merge_trailing_whitespace_appended_to_last_line() {
        let lines = vec![vec![fragment("hello")], vec![fragment(" ")]];
        assert_eq!(
            merge_whitespace_only_lines(&lines),
            vec![vec![fragment("hello"), fragment(" ")]]
        );
    }

    #[test]
    fn merge_carries_multiple_consecutive_whitespace_lines_forward() {
        let lines = vec![
            vec![fragment("hello")],
            vec![fragment(" ")],
            vec![fragment("\t")],
            vec![fragment("world")],
        ];
        assert_eq!(
            merge_whitespace_only_lines(&lines),
            vec![
                vec![fragment("hello")],
                vec![fragment(" "), fragment("\t"), fragment("world")]
            ]
        );
    }

    #[test]
    fn merge_drops_single_space_before_atomic_starting_line() {
        let lines = vec![
            vec![fragment("alpha"), fragment("beta")],
            vec![fragment(" ")],
            vec![fragment("`code`")],
        ];
        assert_eq!(
            merge_whitespace_only_lines(&lines),
            vec![
                vec![fragment("alpha"), fragment("beta")],
                vec![fragment("`code`")]
            ]
        );
    }

    #[test]
    fn merge_empty_input_returns_empty_output() {
        assert!(merge_whitespace_only_lines(&[]).is_empty());
    }

    #[test]
    fn rebalance_moves_atomic_tail_when_fits() {
        let mut lines = vec![
            vec![fragment("alpha"), fragment("`code`")],
            vec![fragment(" "), fragment("tail")],
        ];
        rebalance_atomic_tails(&mut lines, 80);
        assert_eq!(lines[0], vec![fragment("alpha")]);
        assert_eq!(
            lines[1],
            vec![fragment("`code`"), fragment(" "), fragment("tail")]
        );
    }

    #[test]
    fn rebalance_does_not_move_when_overflow() {
        let mut lines = vec![
            vec![fragment("alpha"), fragment("`code`")],
            vec![fragment(" "), fragment("plain")],
        ];
        let original = lines.clone();
        let width = line_width(&lines[1]) + lines[0].last().expect("tail exists").width - 1;
        rebalance_atomic_tails(&mut lines, width);
        assert_eq!(lines, original);
    }

    #[test]
    fn rebalance_skips_when_next_does_not_start_with_space_then_plain() {
        let mut lines = vec![
            vec![fragment("alpha"), fragment("`code`")],
            vec![fragment("plain")],
        ];
        let original = lines.clone();
        rebalance_atomic_tails(&mut lines, 80);
        assert_eq!(lines, original);
    }

    #[test]
    fn rebalance_moves_atomic_tail_at_exact_width_boundary() {
        let mut lines = vec![
            vec![fragment("alpha"), fragment("`tail`")],
            vec![fragment(" "), fragment("beta")],
        ];
        let width = line_width(&lines[1]) + lines[0].last().expect("tail exists").width;
        rebalance_atomic_tails(&mut lines, width);
        assert_eq!(lines[0], vec![fragment("alpha")]);
        assert_eq!(
            lines[1],
            vec![fragment("`tail`"), fragment(" "), fragment("beta")]
        );
    }

    #[test]
    fn rebalance_moves_plain_tail_when_fits() {
        let mut lines = vec![
            vec![fragment("alpha"), fragment("tail")],
            vec![fragment(" "), fragment("beta")],
        ];
        rebalance_atomic_tails(&mut lines, 20);
        assert_eq!(lines[0], vec![fragment("alpha")]);
        assert_eq!(
            lines[1],
            vec![fragment("tail"), fragment(" "), fragment("beta")]
        );
    }

    #[test]
    fn rebalance_leaves_single_plain_fragment_line_unchanged() {
        let mut lines = vec![
            vec![fragment("tail")],
            vec![fragment(" "), fragment("beta")],
        ];
        let original = lines.clone();
        rebalance_atomic_tails(&mut lines, 20);
        assert_eq!(lines, original);
    }

    #[test]
    fn rebalance_ignores_empty_input() {
        let mut lines: Vec<Vec<InlineFragment>> = Vec::new();
        rebalance_atomic_tails(&mut lines, 10);
        assert!(lines.is_empty());
    }

    #[test]
    fn rebalance_leaves_single_line_input_unchanged() {
        let mut lines = vec![vec![fragment("alpha"), fragment("tail")]];
        let original = lines.clone();
        rebalance_atomic_tails(&mut lines, 10);
        assert_eq!(lines, original);
    }

    #[test]
    fn rebalance_skips_when_next_line_starts_with_space_then_atomic() {
        let mut lines = vec![
            vec![fragment("alpha"), fragment("tail")],
            vec![fragment(" "), fragment("`code`")],
        ];
        let original = lines.clone();
        rebalance_atomic_tails(&mut lines, 20);
        assert_eq!(lines, original);
    }
}
