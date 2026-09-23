//! Production consumer decisions derived from the verified classifier.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

use super::{ClassifyCtxKernel, LineClass, classify_seq};

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
