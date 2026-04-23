//! Post-wrap normalization helpers for inline fragment lines.

use super::{FragmentKind, InlineFragment};

fn is_whitespace_only_line(line: &[InlineFragment]) -> bool {
    line.iter().all(InlineFragment::is_whitespace)
}

fn is_single_space_line(line: &[InlineFragment]) -> bool { line.len() == 1 && line[0].text == " " }

fn line_width(line: &[InlineFragment]) -> usize { line.iter().map(|fragment| fragment.width).sum() }

fn line_starts_with_atomic(line: &[InlineFragment]) -> bool {
    line.first().is_some_and(InlineFragment::is_atomic)
}

fn line_starts_with_single_space_then_plain(line: &[InlineFragment]) -> bool {
    line.first()
        .is_some_and(|fragment| fragment.is_whitespace() && fragment.text == " ")
        && line.get(1).is_some_and(InlineFragment::is_plain)
}

fn line_has_rebalanceable_tail(line: &[InlineFragment]) -> bool {
    line.last().is_some_and(InlineFragment::is_atomic)
        || (line.len() > 1 && line.last().is_some_and(InlineFragment::is_plain))
}

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
