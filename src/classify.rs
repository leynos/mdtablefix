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
    OpenFence,
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

/// Classifies one source line using the shared structural precedence.
#[must_use]
pub fn classify_line(line: &str, ctx: &ClassifyCtx) -> LineClass {
    classify_line_with_body(line, ctx).class
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
