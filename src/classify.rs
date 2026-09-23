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
#[derive(Default)]
pub(crate) struct ListContinuationState {
    /// Quote depth and required content indentation of the active list item.
    active: Option<(usize, usize)>,
}

/// Counts structural quote markers in a scanner-bounded prefix.
pub(crate) fn quote_depth(prefix: &str) -> usize {
    prefix.bytes().filter(|byte| *byte == b'>').count()
}

impl ListContinuationState {
    /// Forgets a list at a fence or other explicit block boundary.
    pub(crate) fn reset(&mut self) { self.active = None; }

    /// Records a source line and returns the active list's content indentation.
    pub(crate) fn observe(&mut self, line: &str, classified: &ClassifiedLine<'_>) -> Option<usize> {
        let prefix = &line[..line.len() - classified.body.len()];
        let quote_depth = quote_depth(prefix);
        let indent = structural_content_indent(line, classified.body);

        if classified.class == LineClass::ListItem {
            let marker = classified.body.trim_start_matches([' ', '\t']);
            let marker_len = marker
                .chars()
                .take_while(|ch| !matches!(ch, ' ' | '\t'))
                .count();
            let separator_width = if marker.chars().nth(marker_len) == Some('\t') {
                4 - ((indent + marker_len) % 4)
            } else {
                1
            };
            let required = indent + marker_len + separator_width;
            self.active = Some((quote_depth, required));
            return Some(required);
        }

        if classified.class != LineClass::ParagraphText {
            self.reset();
            return None;
        }

        if let Some((depth, required)) = self.active
            && depth == quote_depth
            && indent >= required
        {
            Some(required)
        } else {
            self.reset();
            None
        }
    }
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
    //! Boundary tests for scalar-to-byte classification offsets.

    use super::*;

    /// Maps a kernel scalar offset onto the matching UTF-8 boundary.
    #[test]
    fn maps_the_kernel_offset_on_a_unicode_prefix_boundary() {
        let classified = classify_line_with_body("> élan", &ClassifyCtx::default());

        assert_eq!(classified.class, LineClass::ParagraphText);
        assert_eq!(classified.body, "élan");
    }
}
