//! Production consumer decisions derived from the verified classifier.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

use super::{ClassifyCtxKernel, LineClass, classify_seq};

verified_kernel_function! {
/// Selects the structural classes that terminate wrapping paragraphs.
///
/// For example, `wrapping_boundary_seq(&['*', '*', '*'],
/// &ClassifyCtxKernel::default())` returns `Some(LineClass::ThematicBreak)`.
#[must_use]
pub(crate) fn wrapping_boundary_seq(
    chars: &[char],
    ctx: &ClassifyCtxKernel,
) -> Option<LineClass>;
ensures(result => result == match crate::spec_classify(chars@, ctx@) {
    LineClass::AtxHeading => Some(LineClass::AtxHeading),
    LineClass::ThematicBreak => Some(LineClass::ThematicBreak),
    LineClass::ListItem => Some(LineClass::ListItem),
    _ => None,
});
{
    match classify_seq(chars, ctx).class {
        LineClass::AtxHeading => Some(LineClass::AtxHeading),
        LineClass::ThematicBreak => Some(LineClass::ThematicBreak),
        LineClass::ListItem => Some(LineClass::ListItem),
        _ => None,
    }
}
}

verified_kernel_function! {
/// Reports whether table buffering may treat a line as structural table input.
///
/// For example, `is_table_class(LineClass::TableDelimiter)` returns `true`.
#[must_use]
pub(crate) fn is_table_class(class: LineClass) -> bool;
ensures(result => result == (
    class == LineClass::TableDelimiter || class == LineClass::TableRow
));
{
    matches!(class, LineClass::TableDelimiter | LineClass::TableRow)
}
}

verified_kernel_function! {
/// Refuses thematic breaks before orphan-specifier syntax is considered.
///
/// For example, `can_be_orphan_specifier_seq(&['*', '*', '*'],
/// &ClassifyCtxKernel::default())` returns `false`.
#[must_use]
pub(crate) fn can_be_orphan_specifier_seq(
    chars: &[char],
    ctx: &ClassifyCtxKernel,
) -> bool;
ensures(result => result == (
    crate::spec_classify(chars@, ctx@) != LineClass::ThematicBreak
));
{
    !matches!(classify_seq(chars, ctx).class, LineClass::ThematicBreak)
}
}

verified_kernel_function! {
/// Accepts a Setext pair only when both structural roles match.
///
/// For example, `is_setext_pair_seq(&['T', 'i', 't', 'l', 'e'],
/// &ClassifyCtxKernel::default(), &['-', '-', '-'],
/// &ClassifyCtxKernel::following(LineClass::ParagraphText, true))` returns
/// `true`.
#[must_use]
pub(crate) fn is_setext_pair_seq(
    candidate: &[char],
    candidate_ctx: &ClassifyCtxKernel,
    underline: &[char],
    underline_ctx: &ClassifyCtxKernel,
) -> bool;
ensures(result => result == (
    crate::spec_classify(candidate@, candidate_ctx@) == LineClass::ParagraphText
        && crate::spec_classify(underline@, underline_ctx@) == LineClass::SetextUnderline
));
{
    is_setext_text_seq(candidate, candidate_ctx)
        && is_setext_underline_seq(underline, underline_ctx)
}
}

verified_kernel_function! {
/// Accepts Setext text only when the shared classifier sees paragraph text.
///
/// For example, `is_setext_text_seq(&['T', 'i', 't', 'l', 'e'],
/// &ClassifyCtxKernel::default())` returns `true`.
#[must_use]
pub(crate) fn is_setext_text_seq(chars: &[char], ctx: &ClassifyCtxKernel) -> bool;
ensures(result => result == (crate::spec_classify(chars@, ctx@) == LineClass::ParagraphText));
{
    matches!(classify_seq(chars, ctx).class, LineClass::ParagraphText)
}
}

verified_kernel_function! {
/// Accepts a Setext underline only under its preceding-paragraph context.
///
/// For example, `is_setext_underline_seq(&['-', '-', '-'],
/// &ClassifyCtxKernel::following(LineClass::ParagraphText, true))` returns
/// `true`.
#[must_use]
pub(crate) fn is_setext_underline_seq(chars: &[char], ctx: &ClassifyCtxKernel) -> bool;
ensures(result => result == (crate::spec_classify(chars@, ctx@) == LineClass::SetextUnderline));
{
    matches!(classify_seq(chars, ctx).class, LineClass::SetextUnderline)
}
}

verified_kernel_function! {
/// Selects only structural thematic breaks for canonicalization.
///
/// For example, `is_canonical_break_seq(&['*', '*', '*'],
/// &ClassifyCtxKernel::default())` returns `true`.
#[must_use]
pub(crate) fn is_canonical_break_seq(chars: &[char], ctx: &ClassifyCtxKernel) -> bool;
ensures(result => result == (crate::spec_classify(chars@, ctx@) == LineClass::ThematicBreak));
{
    matches!(classify_seq(chars, ctx).class, LineClass::ThematicBreak)
}
}

verified_kernel_function! {
/// Confirms that emitted Setext replacement is structurally an ATX heading.
///
/// For example, `is_atx_heading_seq(&['#', ' ', 'T'],
/// &ClassifyCtxKernel::default())` returns `true`.
#[must_use]
pub(crate) fn is_atx_heading_seq(chars: &[char], ctx: &ClassifyCtxKernel) -> bool;
ensures(result => result == (crate::spec_classify(chars@, ctx@) == LineClass::AtxHeading));
{
    matches!(classify_seq(chars, ctx).class, LineClass::AtxHeading)
}
}

verified_loop_function! {
/// Builds the exact seventy-underscore sequence emitted for thematic breaks.
///
/// For example, `canonical_break_chars()` returns seventy underscore
/// characters.
#[must_use]
pub(crate) fn canonical_break_chars() -> Vec<char>;
ensures(result =>
    result@.len() == 70,
    forall|i: int| 0 <= i < result@.len() ==> result@[i] == '_',
);
before {
    let mut result = Vec::new();
    let mut index = 0;
}
while (index < 70) invariant(
    index <= 70,
    result@.len() == index,
    forall|i: int| 0 <= i < result@.len() ==> result@[i] == '_',
) {
    result.push('_');
    index += 1;
}
after { result }
}
