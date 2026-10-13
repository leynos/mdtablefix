//! Production sequence assembly for Setext replacements and their input prefixes.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

use super::{ClassifyCtxKernel, LineClass, classify_seq, predicates::trimmed_range};

verified_kernel_function! {
/// Mirrors Rust's Unicode whitespace predicate for prefix separation.
///
/// For example, `is_unicode_whitespace('\u{00a0}')` returns true, while `>` returns false.
fn is_unicode_whitespace(character: char) -> bool;
ensures(result => result == crate::setext_spec::unicode_whitespace(character));
{
    matches!(character, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{0085}'
        | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}'
        | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}
}

verified_kernel_function! {
/// Determines whether the retained prefix needs a space before the hash run.
///
/// For example, `['>', '>']` returns true, while `['>', ' ']` returns false.
pub(crate) fn needs_space_after_seq(prefix: &[char]) -> bool;
ensures(result => result == crate::setext_spec::needs_space_after(prefix@));
{
    match prefix.len() {
        0 => false,
        length => !is_unicode_whitespace(prefix[length - 1]),
    }
}
}

verified_loop_function! {
/// Appends a sequence without changing any preceding scalar.
///
/// For example, appending `['b']` to `vec!['a']` produces `vec!['a', 'b']`.
fn append_chars(head: Vec<char>, suffix: &[char]) -> Vec<char>;
requires(head@.len() + suffix@.len() <= usize::MAX);
ensures(output => output@ == head@ + suffix@);
before {
    let mut result = head;
    let mut index = 0;
}
ghost_before { let ghost original = result@; }
while (index < suffix.len()) invariant(
    index <= suffix@.len(),
    original.len() + suffix@.len() <= usize::MAX,
    result@ == original + suffix@.subrange(0, index as int),
) {
    result.push(suffix[index]);
    index += 1;
}
proof_loop {
    assert(result@ =~= original + suffix@.subrange(0, index as int));
}
proof_after {
    assert(suffix@.subrange(0, index as int) =~= suffix@);
}
after { result }
}

verified_kernel_function! {
/// Retains a prefix and inserts its quote separator when required.
///
/// For example, `['>', '>']` becomes `vec!['>', '>', ' ']`.
fn separated_prefix(prefix: &[char]) -> Vec<char>;
requires(prefix@.len() + 1 <= usize::MAX);
ensures(result => result@ == crate::setext_spec::separated_prefix(prefix@));
{
    let mut result = append_chars(Vec::new(), prefix);
    if needs_space_after_seq(prefix) {
        result.push(' ');
    }
    result
}
}

verified_kernel_function! {
/// Validates indentation and nested quote markers of the retained input prefix.
///
/// For example, `['>', ' ', '>']` returns true, while four leading spaces return false.
/// The probe contains ordinary text; it does not inspect the constructed heading.
pub(crate) fn valid_setext_prefix(prefix: &[char]) -> bool;
requires(prefix@.len() + 2 <= usize::MAX);
ensures(result => result == crate::setext_spec::valid_prefix(prefix@));
{
    let mut probe = separated_prefix(prefix);
    let boundary = probe.len();
    probe.push('x');
    let ctx = ClassifyCtxKernel {
        is_in_fence: false, open_fence: None, previous: None, prefix_agrees: false,
    };
    let classified = classify_seq(probe.as_slice(), &ctx);
    let (first, _) = trimmed_range(probe.as_slice(), classified.body_start.0);
    matches!(classified.class, LineClass::ParagraphText) && first == boundary
}
}

verified_kernel_function! {
/// Assembles the complete production heading, including prefix and candidate text.
///
/// For example, prefix `['>']`, level two, and text `['T']` produce `> ## T`.
pub(crate) fn convert_setext_seq(prefix: &[char], level: usize, text: &[char]) -> Vec<char>;
requires(
    level == 1 || level == 2,
    prefix@.len() + text@.len() + 4 <= usize::MAX,
    crate::setext_spec::valid_prefix(prefix@),
);
ensures(result =>
    result@ == crate::setext_spec::assembled_heading(prefix@, level as int, text@),
    crate::spec_classify(result@, crate::canonical_context()) == LineClass::AtxHeading,
    crate::spec_classify(result@, crate::canonical_context()) != LineClass::ParagraphText,
    forall|ctx: crate::ClassifyCtxView| !ctx.is_in_fence ==>
        #[trigger] crate::spec_classify(result@, ctx) == LineClass::AtxHeading,
);
before {
    let mut result = separated_prefix(prefix);
    result.push('#');
    if level == 2 {
        result.push('#');
    }
    result.push(' '); // Mandatory ATX separator.
    let result = append_chars(result, text);
}
proof_after {
    crate::setext_spec::lemma_complete_heading_classifies(prefix@, level as int, text@);
}
after { result }
}

verified_kernel_function! {
/// Constructs a replacement only for the admitted production source pair.
///
/// For example, candidate `T`, underline `===` with an agreeing paragraph context,
/// an empty prefix, and level one yield `Some(vec!['#', ' ', 'T'])`.
pub(crate) fn convert_accepted_setext_seq(
    candidate: &[char], candidate_ctx: &ClassifyCtxKernel,
    underline: &[char], underline_ctx: &ClassifyCtxKernel,
    prefix: &[char], level: usize, text: &[char],
) -> Option<Vec<char>>;
requires(level == 1 || level == 2, prefix@.len() + text@.len() + 4 <= usize::MAX);
ensures(result =>
    result.is_some() == (
        crate::spec_classify(candidate@, candidate_ctx@) == LineClass::ParagraphText
        && crate::spec_classify(underline@, underline_ctx@) == LineClass::SetextUnderline
        && crate::setext_spec::valid_prefix(prefix@)
    ),
    match result {
        Some(output) => output@ == crate::setext_spec::assembled_heading(prefix@, level as int, text@)
            && forall|ctx: crate::ClassifyCtxView| !ctx.is_in_fence ==>
                #[trigger] crate::spec_classify(output@, ctx) == LineClass::AtxHeading,
        None => true,
    },
);
{
    if !super::consumers::is_setext_pair_seq(candidate, candidate_ctx, underline, underline_ctx)
        || !valid_setext_prefix(prefix)
    {
        return None;
    }
    Some(convert_setext_seq(prefix, level, text))
}
}
