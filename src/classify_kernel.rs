//! Pure character-sequence kernel for Markdown structural classification.
//!
//! This module intentionally accepts only character slices and explicit
//! context. It has no borrowed-string ranges, regular expressions, I/O, or
//! ambient state, so `verus/lib.rs` can compile this production body directly.

#[path = "classify_kernel_predicates.rs"]
mod predicates;

use predicates::{
    body_starts_with_pipe,
    char_at,
    is_atx_heading,
    is_blank,
    is_closing_fence,
    is_fence_marker,
    is_list_item,
    is_setext_underline,
    is_table_delimiter,
    is_thematic_break,
};

/// A Unicode scalar offset into the character sequence supplied to the kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CharIndex(pub(crate) usize);

/// The structural role of a line after its indentation and blockquote prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LineClass {
    /// Text that can form a paragraph and may be the candidate of a Setext pair.
    ParagraphText,
    /// An ATX heading with a hash run followed by whitespace or end of line.
    AtxHeading,
    /// A `===` or `---` underline whose preceding line is compatible prose.
    SetextUnderline,
    /// A table alignment row containing pipes, colons, dashes, and whitespace.
    TableDelimiter,
    /// A pipe-leading table row that is not an alignment row.
    TableRow,
    /// An opening or closing backtick/tilde fence marker.
    FenceMarker,
    /// A three-or-more marker thematic break.
    ThematicBreak,
    /// An unordered or ordered list item marker followed by whitespace.
    ListItem,
    /// A whitespace-only line.
    Blank,
    /// Source content that must remain literal, such as indented or fenced code.
    Literal,
}

/// Fence state needed to distinguish literal fenced contents from markers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OpenFence {
    /// Character repeated by the fence marker.
    pub(crate) marker: char,
    /// Number of marker characters in the opening fence.
    pub(crate) marker_len: usize,
}

impl OpenFence {
    /// Records the marker that opened the current fenced region.
    #[must_use]
    pub(crate) const fn new(marker: char, marker_len: usize) -> Self { Self { marker, marker_len } }
}

/// Context that makes a line classification independent of its caller.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ClassifyCtxKernel {
    /// Whether the line is within an already-open fenced region.
    pub(crate) is_in_fence: bool,
    /// Opening marker whose compatible closing marker may end that region.
    pub(crate) open_fence: Option<OpenFence>,
    /// Structural class of the immediately preceding source line.
    pub(crate) previous: Option<LineClass>,
    /// Whether the current and preceding lines have identical structural prefixes.
    pub(crate) prefix_agrees: bool,
}

impl ClassifyCtxKernel {
    /// Builds a context for a fenced region.
    #[must_use]
    pub(crate) fn in_fence(open_fence: OpenFence) -> Self {
        Self {
            is_in_fence: true,
            open_fence: Some(open_fence),
            previous: None,
            prefix_agrees: false,
        }
    }

    /// Builds context for the line that follows a classified source line.
    #[must_use]
    pub(crate) const fn following(previous: LineClass, prefix_agrees: bool) -> Self {
        Self {
            is_in_fence: false,
            open_fence: None,
            previous: Some(previous),
            prefix_agrees,
        }
    }
}

/// A structural result and the scalar offset of the scanner's body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct KernelClassification {
    /// Structural class selected by [`classify_seq`].
    pub(crate) class: LineClass,
    /// First scalar after the indentation and blockquote prefix.
    ///
    /// This offset is never greater than the input character-sequence length.
    pub(crate) body_start: CharIndex,
}

/// Classifies one character sequence using the shared structural precedence.
#[must_use]
pub(crate) fn classify_seq(chars: &[char], ctx: &ClassifyCtxKernel) -> KernelClassification {
    if is_blank(chars) {
        return classified(LineClass::Blank, indentation_at(chars, 0).1);
    }

    let (body_start, is_literal) = line_parts(chars);
    if is_literal {
        return classified(LineClass::Literal, body_start);
    }
    if ctx.is_in_fence {
        return classify_within_fence(chars, body_start, ctx);
    }
    classify_open_text(chars, body_start, ctx)
}

/// Classifies a line inside an open fenced region.
fn classify_within_fence(
    chars: &[char],
    body_start: usize,
    ctx: &ClassifyCtxKernel,
) -> KernelClassification {
    if ctx
        .open_fence
        .is_some_and(|open| is_closing_fence(chars, body_start, open))
    {
        return classified(LineClass::FenceMarker, body_start);
    }
    classified(LineClass::Literal, body_start)
}

/// Applies structural precedence outside fenced regions.
fn classify_open_text(
    chars: &[char],
    body_start: usize,
    ctx: &ClassifyCtxKernel,
) -> KernelClassification {
    if is_fence_marker(chars, body_start) {
        return classified(LineClass::FenceMarker, body_start);
    }
    if is_atx_heading(chars, body_start) {
        return classified(LineClass::AtxHeading, body_start);
    }
    if is_table_delimiter(chars, body_start) {
        return classified(LineClass::TableDelimiter, body_start);
    }
    if body_starts_with_pipe(chars, body_start) {
        return classified(LineClass::TableRow, body_start);
    }
    if matches!(ctx.previous, Some(LineClass::ParagraphText))
        && ctx.prefix_agrees
        && is_setext_underline(chars, body_start)
    {
        return classified(LineClass::SetextUnderline, body_start);
    }
    if is_thematic_break(chars, body_start) {
        return classified(LineClass::ThematicBreak, body_start);
    }
    if is_list_item(chars, body_start) {
        return classified(LineClass::ListItem, body_start);
    }
    classified(LineClass::ParagraphText, body_start)
}

/// Constructs a result without allowing the scalar offset to become implicit.
fn classified(class: LineClass, body_start: usize) -> KernelClassification {
    KernelClassification {
        class,
        body_start: CharIndex(body_start),
    }
}

/// Locates the structural body and determines whether it is indented code.
fn line_parts(chars: &[char]) -> (usize, bool) {
    let (outer_width, mut cursor) = indentation_at(chars, 0);
    if outer_width >= 4 {
        return (0, true);
    }

    loop {
        let (indent_width, after_indent) = indentation_at(chars, cursor);
        if indent_width >= 4 || char_at(chars, after_indent) != Some('>') {
            break;
        }
        cursor = after_indent + 1;
        if char_at(chars, cursor) == Some(' ') {
            cursor += 1;
        }
    }

    let (content_indent, _) = indentation_at(chars, cursor);
    (cursor, content_indent >= 4)
}

/// Measures indentation columns and the following scalar offset from `start`.
fn indentation_at(chars: &[char], start: usize) -> (usize, usize) {
    let mut width = 0;
    let mut cursor = start;
    while let Some(character) = char_at(chars, cursor) {
        match character {
            ' ' => {
                width += 1;
                cursor += 1;
            }
            '\t' => {
                width += 4;
                cursor += 1;
            }
            _ => break,
        }
    }
    (width, cursor)
}
