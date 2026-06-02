//! Post-wrap normalisation helpers for inline fragment lines.
//!
//! `wrap_preserving_code` first lets `textwrap` perform greedy line fitting
//! over `InlineFragment` values. That provisional layout can expose
//! whitespace-only lines or leave an atomic tail, such as an inline code span,
//! link, or GFM footnote reference, stranded at the end of the previous line.
//!
//! This module repairs those artefacts before rendering text back to Markdown.
//! It depends on the fragment classification from `inline::fragment`, but it
//! does not re-tokenize source text; its job is only to preserve the wrapper's
//! historical whitespace behaviour and keep eligible atomic fragments attached
//! to the line where they fit.

use tracing::trace;

use super::fragment::{FragmentKind, InlineFragment};

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
    (line.len() > 1 && line.last().is_some_and(InlineFragment::is_atomic))
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
        debug_assert!(
            false,
            "inline code tail vanished after successful tail-kind check"
        );
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
            trace!(
                index,
                fragment_count = line.len(),
                "normalising whitespace-only wrapped line"
            );
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
#[path = "postprocess_tests.rs"]
mod tests;
