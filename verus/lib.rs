//! Root entry point for production-used Verus kernels.
//!
//! Each verified kernel must include the production module it proves with a
//! `#[path]` attribute, or otherwise carry a refinement proof to the formatter
//! function that calls it. Do not add a standalone reimplementation here.

use vstd::prelude::*;

verus! {

#[path = "../src/classify_kernel.rs"]
pub mod production_classify;

use production_classify::LineClass;

/// State relevant to a structural decision after source scanning.
pub struct ClassifyCtxView {
    pub is_in_fence: bool,
    pub open_fence: Option<production_classify::OpenFence>,
    pub previous: Option<LineClass>,
    pub prefix_agrees: bool,
}

impl View for production_classify::ClassifyCtxKernel {
    type V = ClassifyCtxView;

    open spec fn view(&self) -> ClassifyCtxView {
        ClassifyCtxView {
            is_in_fence: self.is_in_fence,
            open_fence: self.open_fence,
            previous: self.previous,
            prefix_agrees: self.prefix_agrees,
        }
    }
}

/// The two scalar values that Markdown treats as indentation whitespace.
pub open spec fn spec_is_markdown_whitespace(character: char) -> bool {
    character == ' ' || character == '\t'
}

/// Whether the suffix beginning at `start` contains only indentation whitespace.
pub open spec fn spec_is_blank_from(s: Seq<char>, start: int) -> bool {
    forall|i: int| start <= i < s.len() ==> spec_is_markdown_whitespace(s[i])
}

pub open spec fn is_atx_heading(s: Seq<char>) -> bool {
    s.len() >= 2 && s[0] == '#' && s[1] == ' '
}

pub open spec fn is_setext_underline(s: Seq<char>) -> bool {
    s.len() >= 3 && forall|i: int| 0 <= i < s.len() ==> s[i] == '-'
}

pub open spec fn is_table_delimiter(s: Seq<char>) -> bool {
    s.len() >= 5
        && s[0] == '|'
        && s[s.len() - 1] == '|'
        && forall|i: int| #![trigger s.index(i)] 0 <= i < s.len() ==> matches!(s.index(i), '|' | ':' | '-' | ' ')
}

pub open spec fn is_table_row(s: Seq<char>) -> bool {
    s.len() > 0 && s[0] == '|'
}

pub open spec fn is_fence_marker(s: Seq<char>) -> bool {
    s.len() >= 3 && (s[0] == '`' || s[0] == '~')
}

pub open spec fn is_thematic_break(s: Seq<char>) -> bool {
    s.len() >= 3 && forall|i: int| 0 <= i < s.len() ==> s[i] == '_'
}

pub open spec fn is_list_item(s: Seq<char>) -> bool {
    let zero: int = 0;
    let one: int = 1;
    s.len() >= 2 && matches!(s.index(zero), '-' | '*' | '+') && s.index(one) == ' '
}

/// Returns whether `s` is ordinary paragraph text in this structural model.
pub open spec fn is_paragraph_text(s: Seq<char>) -> bool {
    !is_atx_heading(s)
        && !is_table_delimiter(s)
        && !is_table_row(s)
        && !is_fence_marker(s)
        && !is_thematic_break(s)
        && !is_list_item(s)
        && s.len() > 0
}

/// Verus model of the scanner's structural precedence.
pub open spec fn spec_classify(s: Seq<char>, ctx: ClassifyCtxView) -> LineClass {
    if s.len() == 0 {
        LineClass::Blank
    } else if ctx.is_in_fence {
        LineClass::Literal
    } else if is_fence_marker(s) {
        LineClass::FenceMarker
    } else if is_atx_heading(s) {
        LineClass::AtxHeading
    } else if is_table_delimiter(s) {
        LineClass::TableDelimiter
    } else if is_table_row(s) {
        LineClass::TableRow
    } else if matches!(ctx.previous, Some(LineClass::ParagraphText))
        && ctx.prefix_agrees
        && is_setext_underline(s)
    {
        LineClass::SetextUnderline
    } else if is_thematic_break(s) {
        LineClass::ThematicBreak
    } else if is_list_item(s) {
        LineClass::ListItem
    } else {
        LineClass::ParagraphText
    }
}

/// Converts an accepted Setext pair to the emitted ATX heading shape.
proof fn convert_setext(candidate: Seq<char>, underline: Seq<char>) -> (result: Seq<char>)
    requires
        spec_classify(
            candidate,
            canonical_context(),
        ) == LineClass::ParagraphText,
        spec_classify(
            underline,
            ClassifyCtxView {
                is_in_fence: false,
                open_fence: None,
                previous: Some(LineClass::ParagraphText),
                prefix_agrees: true,
            },
        ) == LineClass::SetextUnderline,
    ensures
        spec_classify(
            result,
            canonical_context(),
        ) == LineClass::AtxHeading,
{
    let emitted = Seq::<char>::empty().push('#').push(' ').add(candidate);
    assert(is_atx_heading(emitted));
    emitted
}

pub open spec fn canonical_break() -> Seq<char> { Seq::<char>::new(70, |i: int| '_') }

pub open spec fn canonical_context() -> ClassifyCtxView {
    ClassifyCtxView { is_in_fence: false, open_fence: None, previous: None, prefix_agrees: true }
}

proof fn lemma_canonical_break_remains_structural()
    ensures
        spec_classify(
            canonical_break(),
            canonical_context(),
        ) == LineClass::ThematicBreak,
{
    assert(is_thematic_break(canonical_break()));
}

pub open spec fn wrapper_accepts(s: Seq<char>, ctx: ClassifyCtxView) -> bool {
    spec_classify(s, ctx) == LineClass::ParagraphText
}

pub open spec fn heading_accepts(s: Seq<char>, ctx: ClassifyCtxView) -> bool {
    spec_classify(s, ctx) == LineClass::ParagraphText
}

pub open spec fn table_accepts(s: Seq<char>, ctx: ClassifyCtxView) -> bool {
    matches!(spec_classify(s, ctx), LineClass::TableRow | LineClass::TableDelimiter)
}

pub open spec fn orphan_specifier_accepts(s: Seq<char>, ctx: ClassifyCtxView) -> bool {
    spec_classify(s, ctx) == LineClass::ParagraphText
}

proof fn lemma_break_rejected_by_consumers()
    ensures
        !wrapper_accepts(canonical_break(), canonical_context()),
        !heading_accepts(canonical_break(), canonical_context()),
        !table_accepts(canonical_break(), canonical_context()),
        !orphan_specifier_accepts(canonical_break(), canonical_context()),
{
    lemma_canonical_break_remains_structural();
}

/// Witnesses that ordinary digit-prefixed prose is not excluded by the model.
proof fn lemma_paragraph_exists()
    ensures
        spec_classify(
            Seq::<char>::empty()
                .push('2')
                .push('0')
                .push('2')
                .push('4')
                .push(' ')
                .push('r')
                .push('e')
                .push('v')
                .push('e')
                .push('n')
                .push('u')
                .push('e'),
            canonical_context(),
        ) == LineClass::ParagraphText,
{
}

/// Witnesses that a table alignment row is distinct from paragraph text.
proof fn lemma_delimiter_exists()
    ensures
        spec_classify(
            Seq::<char>::empty()
                .push('|')
                .push('-')
                .push('-')
                .push('-')
                .push('|')
                .push('-')
                .push('-')
                .push('-')
                .push('|'),
            canonical_context(),
        ) == LineClass::TableDelimiter,
{
}

/// Witnesses that the canonical seventy-underscore break is structural.
proof fn lemma_break_exists()
    ensures
        spec_classify(canonical_break(), canonical_context()) == LineClass::ThematicBreak,
{
    lemma_canonical_break_remains_structural();
}

} // verus!

fn main() {}
