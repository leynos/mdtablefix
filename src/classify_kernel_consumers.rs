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
#[must_use]
pub(crate) fn is_setext_text_seq(chars: &[char], ctx: &ClassifyCtxKernel) -> bool;
ensures(result => result == (crate::spec_classify(chars@, ctx@) == LineClass::ParagraphText));
{
    matches!(classify_seq(chars, ctx).class, LineClass::ParagraphText)
}
}

verified_kernel_function! {
/// Accepts a Setext underline only under its preceding-paragraph context.
#[must_use]
pub(crate) fn is_setext_underline_seq(chars: &[char], ctx: &ClassifyCtxKernel) -> bool;
ensures(result => result == (crate::spec_classify(chars@, ctx@) == LineClass::SetextUnderline));
{
    matches!(classify_seq(chars, ctx).class, LineClass::SetextUnderline)
}
}

verified_kernel_function! {
/// Selects only structural thematic breaks for canonicalization.
#[must_use]
pub(crate) fn is_canonical_break_seq(chars: &[char], ctx: &ClassifyCtxKernel) -> bool;
ensures(result => result == (crate::spec_classify(chars@, ctx@) == LineClass::ThematicBreak));
{
    matches!(classify_seq(chars, ctx).class, LineClass::ThematicBreak)
}
}

verified_kernel_function! {
/// Confirms that emitted Setext replacement is structurally an ATX heading.
#[must_use]
pub(crate) fn is_atx_heading_seq(chars: &[char], ctx: &ClassifyCtxKernel) -> bool;
ensures(result => result == (crate::spec_classify(chars@, ctx@) == LineClass::AtxHeading));
{
    matches!(classify_seq(chars, ctx).class, LineClass::AtxHeading)
}
}

verified_kernel_function! {
/// Builds the production ATX marker for a level-one or level-two Setext heading.
#[must_use]
pub(crate) fn setext_atx_marker(level: usize) -> Vec<char>;
requires(level == 1 || level == 2);
ensures(result =>
    result@.len() == level + 1,
    result@[level as int] == ' ',
    forall|i: int| 0 <= i < level ==> result@[i] == '#',
);
{
    let mut marker = Vec::new();
    marker.push('#');
    if level == 2 {
        marker.push('#');
    }
    marker.push(' ');
    marker
}
}

verified_loop_function! {
/// Builds the exact seventy-underscore sequence emitted for thematic breaks.
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
