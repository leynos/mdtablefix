//! Joins inline fragments that the source writes with no whitespace between.
//!
//! The wrapper may break a line only where the source already has
//! whitespace, because Markdown renders a soft line break as a space. This
//! module turns that rule into the fragment stream's shape: after grouping,
//! any two neighbouring non-whitespace fragments become one, leaving the
//! whitespace fragments as the only places `textwrap` can break.

use super::fragment::InlineFragment;

/// Joins fragments that meet with no whitespace between them.
///
/// Markdown renders a soft line break as a space, so a break is only safe where
/// the source already has whitespace. Grouping couples most attached
/// constructs, but not every shape: `(` before a reference link, a code span
/// after `/`, `-[` before prose, or `**` before a code span each reached the
/// wrapper as two fragments with nothing between them. Merging every touching
/// pair here makes the whitespace fragments the only break opportunities, so
/// wrapping can never insert a space into the rendered text.
pub(super) fn join_touching_fragments(fragments: Vec<InlineFragment>) -> Vec<InlineFragment> {
    let mut joined: Vec<InlineFragment> = Vec::with_capacity(fragments.len());
    for fragment in fragments {
        match joined.last_mut() {
            Some(previous) if touches(previous, &fragment) => {
                let text = format!("{}{}", previous.text, fragment.text);
                *previous = InlineFragment::new(text);
            }
            _ => joined.push(fragment),
        }
    }
    joined
}

/// Returns whether `next` follows `previous` with no whitespace between them.
///
/// Grouping sometimes couples a leading space into a fragment (a bare bracket
/// reference carries the space before it), so the test reads the text at the
/// seam rather than the fragment kinds.
fn touches(previous: &InlineFragment, next: &InlineFragment) -> bool {
    let previous_ends_solid = previous
        .text
        .chars()
        .next_back()
        .is_some_and(|c| !c.is_whitespace());
    let next_starts_solid = next.text.chars().next().is_some_and(|c| !c.is_whitespace());
    previous_ends_solid && next_starts_solid
}
