//! Boundary adapters for the production-used structural scanner kernel.
//!
//! [`classify_line`] converts a source line to Unicode scalar values once and
//! delegates every classification decision to [`classify_seq`]. The kernel
//! reports scalar offsets; this boundary maps them back to byte offsets only
//! after checking the original UTF-8 boundary.

pub(crate) use crate::classify_kernel::{
    CharIndex,
    ClassifyCtxKernel as ClassifyCtx,
    KernelClassification,
    LineClass,
    classify_seq,
};

/// A source-line classification with its structural body at a UTF-8 boundary.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ClassifiedLine<'line> {
    /// Structural class selected by the executable scanner kernel.
    pub(crate) class: LineClass,
    /// Content after the indentation and blockquote prefix.
    pub(crate) body: &'line str,
}

/// Minimal list indentation state shared by Setext and break consumers.
///
/// Paragraph lines can continue an item lazily without the item indentation;
/// the remembered content column still decides whether a later underline
/// belongs to that list item.
#[derive(Default)]
pub(crate) struct ListContinuationState {
    /// Quote depth and required content indentation of the active list item.
    active: Option<(usize, usize)>,
    /// A blank line requires the next item continuation to be indented.
    has_blank_line: bool,
}

/// Counts structural quote markers in a scanner-bounded prefix.
pub(crate) fn quote_depth(prefix: &str) -> usize {
    prefix.bytes().filter(|byte| *byte == b'>').count()
}

impl ListContinuationState {
    /// Forgets a list at a fence or other explicit block boundary.
    pub(crate) fn reset(&mut self) {
        self.active = None;
        self.has_blank_line = false;
    }

    /// Records a source line and returns the active list's content indentation.
    pub(crate) fn observe(&mut self, line: &str, classified: &ClassifiedLine<'_>) -> Option<usize> {
        let prefix = &line[..line.len() - classified.body.len()];
        let quote_depth = quote_depth(prefix);
        let indent = structural_content_indent(line, classified.body);

        if classified.class == LineClass::Blank {
            self.has_blank_line = self.active.is_some();
            return None;
        }

        if classified.class == LineClass::ListItem {
            let required = list_content_indent(classified.body, indent);
            self.active = Some((quote_depth, required));
            self.has_blank_line = false;
            return Some(required);
        }

        if let Some(required) = self.continuation_indent(classified, quote_depth, indent) {
            self.has_blank_line = false;
            Some(required)
        } else {
            self.reset();
            None
        }
    }

    /// Returns the active item's content column when `classified` continues it.
    ///
    /// The line's quote depth must match the item's. Paragraph text may
    /// continue lazily, without reaching the content column, but only before an
    /// intervening blank line: an outdented line after a blank line has left
    /// the list. Literal content always has to reach the content column,
    /// because indented code is not a lazy continuation.
    fn continuation_indent(
        &self,
        classified: &ClassifiedLine<'_>,
        quote_depth: usize,
        indent: usize,
    ) -> Option<usize> {
        let (depth, required) = self.active?;
        if depth != quote_depth {
            return None;
        }
        match classified.class {
            LineClass::ParagraphText if !self.has_blank_line => Some(required),
            LineClass::ParagraphText | LineClass::Literal if indent >= required => Some(required),
            _ => None,
        }
    }
}

/// Calculates the content column after a list marker and its separator.
fn list_content_indent(body: &str, indent: usize) -> usize {
    let marker = body.trim_start_matches([' ', '\t']);
    let marker_len = marker
        .chars()
        .take_while(|ch| !matches!(ch, ' ' | '\t'))
        .count();
    let marker_end = indent + marker_len;
    let mut content_column = marker_end;
    for ch in marker.chars().skip(marker_len) {
        match ch {
            ' ' => content_column += 1,
            '\t' => content_column += 4 - content_column % 4,
            _ => break,
        }
    }
    let separator_width = content_column - marker_end;
    let separator_width = if (1..=4).contains(&separator_width) {
        separator_width
    } else {
        1
    };
    marker_end + separator_width
}

/// Measures indentation after blockquote markers, or at the outer line edge.
pub(crate) fn structural_content_indent(line: &str, body: &str) -> usize {
    let prefix = &line[..line.len() - body.len()];
    let source = if prefix.contains('>') { body } else { line };
    let mut columns = 0;
    for ch in source.chars() {
        match ch {
            ' ' => columns += 1,
            '\t' => columns += 4 - columns % 4,
            _ => break,
        }
    }
    columns
}

/// Classifies one source line using the shared structural precedence.
#[must_use]
pub fn classify_line(line: &str, ctx: &ClassifyCtx) -> LineClass {
    classify_line_with_body(line, ctx).class
}

/// Tests whether a Setext candidate is paragraph text through the verified kernel.
#[must_use]
pub(crate) fn is_setext_text_line(line: &str, ctx: &ClassifyCtx) -> bool {
    let chars = line.chars().collect::<Vec<_>>();
    crate::classify_kernel::consumers::is_setext_text_seq(&chars, ctx)
}

/// Tests whether a Setext underline follows compatible paragraph text.
#[must_use]
pub(crate) fn is_setext_underline_line(line: &str, ctx: &ClassifyCtx) -> bool {
    let chars = line.chars().collect::<Vec<_>>();
    crate::classify_kernel::consumers::is_setext_underline_seq(&chars, ctx)
}

/// Tests whether a line should become the canonical thematic break.
#[must_use]
pub(crate) fn is_canonical_break_line(line: &str, ctx: &ClassifyCtx) -> bool {
    let chars = line.chars().collect::<Vec<_>>();
    crate::classify_kernel::consumers::is_canonical_break_seq(&chars, ctx)
}

/// Confirms that a generated Setext replacement is an ATX heading.
#[must_use]
pub(crate) fn is_atx_heading_line(line: &str, ctx: &ClassifyCtx) -> bool {
    let chars = line.chars().collect::<Vec<_>>();
    crate::classify_kernel::consumers::is_atx_heading_seq(&chars, ctx)
}

/// Classifies a source line and maps the kernel's scalar offset to UTF-8.
#[must_use]
pub(crate) fn classify_line_with_body<'line>(
    line: &'line str,
    ctx: &ClassifyCtx,
) -> ClassifiedLine<'line> {
    let chars = line.chars().collect::<Vec<_>>();
    let KernelClassification { class, body_start } = classify_seq(&chars, ctx);
    let body_start = byte_offset_at_char_index(line, body_start);

    ClassifiedLine {
        class,
        body: &line[body_start..],
    }
}

/// Maps a Unicode scalar offset to the corresponding checked UTF-8 byte offset.
fn byte_offset_at_char_index(line: &str, CharIndex(target): CharIndex) -> usize {
    debug_assert!(target <= line.chars().count());
    line.char_indices()
        .nth(target)
        .map_or(line.len(), |(byte_offset, _)| byte_offset)
}

#[cfg(test)]
mod tests {
    //! Boundary tests for scalar-to-byte classification offsets, and
    //! state-transition tests for list continuation eligibility.

    use rstest::rstest;

    use super::*;

    /// Maps a kernel scalar offset onto the matching UTF-8 boundary.
    #[test]
    fn maps_the_kernel_offset_on_a_unicode_prefix_boundary() {
        let classified = classify_line_with_body("> élan", &ClassifyCtx::default());

        assert_eq!(classified.class, LineClass::ParagraphText);
        assert_eq!(classified.body, "élan");
    }

    /// Observes one line through the default classifier context.
    fn observe(state: &mut ListContinuationState, line: &str) -> Option<usize> {
        let classified = classify_line_with_body(line, &ClassifyCtx::default());
        state.observe(line, &classified)
    }

    /// A new item records its content column and reports it.
    #[test]
    fn list_item_sets_the_content_column() {
        let mut state = ListContinuationState::default();

        assert_eq!(observe(&mut state, "- item"), Some(2));
        assert_eq!(state.active, Some((0, 2)));
    }

    /// An outdented paragraph still continues the item before a blank line.
    #[test]
    fn lazy_paragraph_continues_before_a_blank_line() {
        let mut state = ListContinuationState::default();
        observe(&mut state, "- item");

        // No indentation at all, but no blank line either: lazy continuation.
        assert_eq!(observe(&mut state, "continuation"), Some(2));
    }

    /// A blank line ends the item, so the next outdented paragraph has left it.
    #[test]
    fn paragraph_after_blank_line_requires_the_content_column() {
        let mut state = ListContinuationState::default();
        observe(&mut state, "- item");
        assert_eq!(observe(&mut state, ""), None);

        assert_eq!(observe(&mut state, "continuation"), None);
        assert!(state.active.is_none(), "the outdented line left the list");
    }

    /// An indented paragraph still continues the item after a blank line.
    #[test]
    fn indented_paragraph_continues_after_blank_line() {
        let mut state = ListContinuationState::default();
        observe(&mut state, "- item");
        observe(&mut state, "");

        assert_eq!(observe(&mut state, "  continuation"), Some(2));
        assert!(!state.has_blank_line, "continuing clears the blank marker");
    }

    /// A quote-depth change ends the item rather than continuing it.
    #[rstest]
    #[case("- item", "> continuation")]
    #[case("> - item", "continuation")]
    fn quote_depth_change_ends_the_item(#[case] item: &str, #[case] next: &str) {
        let mut state = ListContinuationState::default();
        assert_eq!(observe(&mut state, item), Some(2));

        assert_eq!(observe(&mut state, next), None);
        assert!(
            state.active.is_none(),
            "a changed quote depth must reset the item"
        );
    }

    /// A new item after a blank line restarts the list.
    #[test]
    fn new_item_after_blank_line_restarts_the_list() {
        let mut state = ListContinuationState::default();
        observe(&mut state, "- item");
        observe(&mut state, "");

        assert_eq!(observe(&mut state, "- second"), Some(2));
        assert_eq!(state.active, Some((0, 2)));
        assert!(!state.has_blank_line, "starting an item clears the marker");
    }

    /// Four columns of indentation classifies as literal content.
    #[test]
    fn literal_content_at_the_content_column_continues_the_item() {
        let mut state = ListContinuationState::default();
        observe(&mut state, "- item");

        assert_eq!(observe(&mut state, "    code"), Some(2));
    }

    /// Indented code below the content column ends the item.
    #[test]
    fn literal_content_below_the_content_column_ends_the_item() {
        let mut state = ListContinuationState::default();
        // A wide separator pushes the content column to five, past the
        // indented-code threshold of four.
        assert_eq!(observe(&mut state, "   - deep"), Some(5));

        assert_eq!(observe(&mut state, "    code"), None);
        assert!(state.active.is_none(), "shallow code left the item");
    }

    /// A blank line with no active item does not arm the blank marker.
    #[test]
    fn blank_line_without_an_item_leaves_the_marker_clear() {
        let mut state = ListContinuationState::default();

        assert_eq!(observe(&mut state, ""), None);
        assert!(!state.has_blank_line);
    }

    /// A fence boundary forgets the item and the blank marker.
    #[test]
    fn reset_forgets_the_item_and_blank_marker() {
        let mut state = ListContinuationState::default();
        observe(&mut state, "- item");
        observe(&mut state, "");

        state.reset();

        assert_eq!(state.active, None);
        assert!(!state.has_blank_line);
    }
}
