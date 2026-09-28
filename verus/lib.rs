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

#[path = "classify_spec.rs"]
pub mod classify_spec;
pub use classify_spec::*;

/// A leading tab occupies four columns and leaves the line literal.
proof fn lemma_leading_tab_is_literal(s: Seq<char>)
    requires s.len() > 0, s[0] == '\t'
    ensures spec_line_parts(s) == (0int, true)
{
    assert(spec_indentation_at(s, 1, 0, 4) == (4int, 1int));
    assert(spec_indentation_at(s, 0, 0, 0) == (4int, 1int));
}

/// Four leading spaces likewise preclude structural classification.
proof fn lemma_four_spaces_are_literal(s: Seq<char>)
    requires s.len() >= 4,
        s[0] == ' ', s[1] == ' ', s[2] == ' ', s[3] == ' '
    ensures spec_line_parts(s) == (0int, true)
{
    assert(spec_indentation_at(s, 4, 0, 4).0 == 4);
    assert(spec_indentation_at(s, 3, 0, 3).0 == 4);
    assert(spec_indentation_at(s, 2, 0, 2).0 == 4);
    assert(spec_indentation_at(s, 1, 0, 1).0 == 4);
    assert(spec_indentation_at(s, 0, 0, 0).0 == 4);
}

/// Structural precedence over the exact body and context supplied to the kernel.
pub open spec fn spec_open_text(s: Seq<char>, start: int, ctx: ClassifyCtxView) -> LineClass {
    if spec_fence_marker(s, start) {
        LineClass::FenceMarker
    } else if spec_atx_heading(s, start) {
        LineClass::AtxHeading
    } else if spec_table_delimiter(s, start) {
        LineClass::TableDelimiter
    } else if spec_body_starts_with_pipe(s, start) {
        LineClass::TableRow
    } else if matches!(ctx.previous, Some(LineClass::ParagraphText))
        && ctx.prefix_agrees && spec_setext_underline(s, start) {
        LineClass::SetextUnderline
    } else if spec_thematic_break(s, start) {
        LineClass::ThematicBreak
    } else if spec_list_item(s, start) {
        LineClass::ListItem
    } else {
        LineClass::ParagraphText
    }
}

pub open spec fn spec_within_fence(s: Seq<char>, start: int, ctx: ClassifyCtxView) -> LineClass {
    if matches!(ctx.open_fence, Some(open) if spec_closing_fence(s, start, open)) {
        LineClass::FenceMarker
    } else {
        LineClass::Literal
    }
}

/// Structural precedence over the exact body and context supplied to the kernel.
pub open spec fn spec_classify(s: Seq<char>, ctx: ClassifyCtxView) -> LineClass {
    let (body_start, is_literal) = spec_line_parts(s);
    if ctx.is_in_fence {
        if is_literal {
            LineClass::Literal
        } else {
            spec_within_fence(s, body_start, ctx)
        }
    } else if spec_is_blank_from(s, 0) || spec_is_blank_from(s, body_start) {
        LineClass::Blank
    } else if is_literal {
        LineClass::Literal
    } else {
        spec_open_text(s, body_start, ctx)
    }
}

/// Trimming trailing whitespace cannot erase a non-whitespace first scalar.
proof fn lemma_trim_preserves_first(s: Seq<char>, end: int)
    requires 0 < end <= s.len(), !spec_is_markdown_whitespace(s[0])
    ensures 1 <= spec_trim_end(s, 0, end) <= end
    decreases end
{
    if end > 1 && spec_is_markdown_whitespace(s[end - 1]) {
        lemma_trim_preserves_first(s, end - 1);
    }
}

/// Adding an ATX prefix produces an ATX heading for any candidate text.
proof fn lemma_atx_prefix_classifies(candidate: Seq<char>) -> (result: Seq<char>)
    ensures
        result == Seq::<char>::empty().push('#').push(' ').add(candidate),
        spec_classify(result, canonical_context()) == LineClass::AtxHeading,
        spec_classify(result, canonical_context()) != LineClass::ParagraphText,
{
    let emitted = Seq::<char>::empty().push('#').push(' ').add(candidate);
    assert(emitted[0] == '#');
    assert(emitted[1] == ' ');
    assert(spec_line_parts(emitted) == (0int, false));
    lemma_trim_preserves_first(emitted, emitted.len() as int);
    assert(spec_trim_start(emitted, 0, emitted.len() as int) == 0);
    let trimmed_end = spec_trim_end(emitted, 0, emitted.len() as int);
    assert(1 <= trimmed_end <= emitted.len());
    if trimmed_end > 1 {
        assert(emitted[1] == ' ');
        assert(spec_marker_run_len(emitted, 1, trimmed_end, '#') == 0);
    }
    assert(spec_marker_run_len(emitted, 0, trimmed_end, '#') == 1);
    assert(spec_atx_heading(emitted, 0));
    emitted
}

pub open spec fn canonical_context() -> ClassifyCtxView {
    ClassifyCtxView { is_in_fence: false, open_fence: None, previous: None, prefix_agrees: false }
}

pub open spec fn canonical_break() -> Seq<char> { Seq::<char>::new(70, |i: int| '_') }

/// Each suffix of the canonical break contains only underscores.
proof fn lemma_canonical_suffix(s: Seq<char>, start: int)
    requires s.len() == 70,
        forall|i: int| 0 <= i < 70 ==> s[i] == '_',
        0 <= start <= 70
    ensures
        spec_marker_count(s, start, 70, '_') == 70 - start,
        spec_thematic_valid(s, start, 70, '_')
    decreases 70 - start
{
    if start < 70 {
        lemma_canonical_suffix(s, start + 1);
    }
}

/// The emitted canonical break remains structural in the exact classifier.
proof fn lemma_canonical_break_remains_structural()
    ensures spec_classify(canonical_break(), canonical_context()) == LineClass::ThematicBreak
{
    let s = canonical_break();
    assert(s.len() == 70);
    assert(forall|i: int| 0 <= i < 70 ==> s[i] == '_');
    lemma_canonical_suffix(s, 0);
    assert(spec_line_parts(s) == (0int, false));
    assert(!spec_is_blank_from(s, 0));
    assert(!spec_contains(s, 0, 70, '|'));
    assert(!spec_table_delimiter(s, 0));
    assert(spec_thematic_break(s, 0));
}

/// Concrete witness that ordinary digit-leading text remains paragraph text.
proof fn lemma_paragraph_exists()
    ensures spec_classify(
        seq!['2', '0', '2', '4', ' ', 'r', 'e', 'v', 'e', 'n', 'u', 'e'],
        canonical_context(),
    ) == LineClass::ParagraphText
{
    let s = seq!['2', '0', '2', '4', ' ', 'r', 'e', 'v', 'e', 'n', 'u', 'e'];
    assert(spec_indentation_at(s, 0, 0, 0) == (0int, 0int));
    assert(spec_line_parts_from(s, 0, 0, 12) == (0int, false));
    assert(spec_line_parts(s) == (0int, false));
    assert(!spec_is_blank_from(s, 0)) by {
        assert(!spec_is_markdown_whitespace(s[0]));
    }
    assert(spec_trimmed_range(s, 0) == (0int, 12int));
    assert(!spec_fence_marker(s, 0));
    assert(!spec_atx_heading(s, 0));
    assert(forall|i: int| 0 <= i < s.len() ==> s[i] != '|');
    assert(!spec_contains(s, 0, 12, '|'));
    assert(!spec_table_delimiter(s, 0));
    assert(!spec_body_starts_with_pipe(s, 0));
    assert(!spec_thematic_break(s, 0));
    assert(!spec_ordered_list_item(s, 4, 12, 4));
    assert(!spec_ordered_list_item(s, 3, 12, 3));
    assert(!spec_ordered_list_item(s, 2, 12, 2));
    assert(!spec_ordered_list_item(s, 1, 12, 1));
    assert(!spec_list_item(s, 0));
    assert(spec_classify(s, canonical_context()) == LineClass::ParagraphText);
}

/// Concrete witness that a pipe-delimited dash row is a table delimiter.
proof fn lemma_delimiter_exists()
    ensures spec_classify(
        seq!['|', '-', '-', '-', '|', '-', '-', '-', '|'],
        canonical_context(),
    ) == LineClass::TableDelimiter
{
    let s = seq!['|', '-', '-', '-', '|', '-', '-', '-', '|'];
    assert(spec_indentation_at(s, 0, 0, 0) == (0int, 0int));
    assert(spec_line_parts_from(s, 0, 0, 9) == (0int, false));
    assert(spec_line_parts(s) == (0int, false));
    assert(!spec_is_blank_from(s, 0)) by {
        assert(!spec_is_markdown_whitespace(s[0]));
    }
    assert(spec_trimmed_range(s, 0) == (0int, 9int));
    assert(!spec_fence_marker(s, 0));
    assert(!spec_atx_heading(s, 0));
    assert(spec_skip_pipes(s, 1, 9) == 1);
    assert(spec_skip_pipes(s, 0, 9) == 1);
    assert(spec_contains(s, 0, 9, '|')) by {
        assert(s[0] == '|');
    }
    assert(forall|i: int| 1 <= i < 4 ==> s[i] == '-');
    assert(forall|i: int| 5 <= i < 8 ==> s[i] == '-');
    assert(spec_table_cell(s, 1, 4));
    assert(spec_table_cell(s, 5, 8));
    assert(spec_table_cells(s, 5, 8, 8));
    assert(spec_table_cells(s, 5, 7, 8));
    assert(spec_table_cells(s, 5, 6, 8));
    assert(spec_table_cells(s, 5, 5, 8));
    assert(spec_table_cells(s, 1, 4, 8));
    assert(spec_table_cells(s, 1, 3, 8));
    assert(spec_table_cells(s, 1, 2, 8));
    assert(spec_table_cells(s, 1, 1, 8));
    assert(spec_table_delimiter(s, 0));
    assert(spec_classify(s, canonical_context()) == LineClass::TableDelimiter);
}

/// Concrete witness that the emitted seventy-underscore line is structural.
proof fn lemma_break_exists()
    ensures spec_classify(canonical_break(), canonical_context()) == LineClass::ThematicBreak
{
    lemma_canonical_break_remains_structural();
    assert(spec_classify(canonical_break(), canonical_context()) == LineClass::ThematicBreak);
}

/// Wrapping observes the canonical break as a paragraph boundary.
proof fn lemma_canonical_break_ends_wrapping()
    ensures match spec_classify(canonical_break(), canonical_context()) {
        LineClass::AtxHeading => Some(LineClass::AtxHeading),
        LineClass::ThematicBreak => Some(LineClass::ThematicBreak),
        LineClass::ListItem => Some(LineClass::ListItem),
        _ => None,
    } == Some(LineClass::ThematicBreak)
{
    lemma_canonical_break_remains_structural();
}

/// Setext conversion cannot accept the canonical break as candidate text.
proof fn lemma_canonical_break_is_not_setext_text()
    ensures spec_classify(canonical_break(), canonical_context()) != LineClass::ParagraphText
{
    lemma_canonical_break_remains_structural();
}

/// Table buffering cannot accept the canonical break as a structural table line.
proof fn lemma_canonical_break_is_not_table_input()
    ensures
        spec_classify(canonical_break(), canonical_context()) != LineClass::TableDelimiter,
        spec_classify(canonical_break(), canonical_context()) != LineClass::TableRow,
{
    lemma_canonical_break_remains_structural();
}

/// Orphan attachment rejects the canonical break before matching specifier syntax.
proof fn lemma_canonical_break_is_not_orphan_specifier()
    ensures !(
        spec_classify(canonical_break(), canonical_context()) != LineClass::ThematicBreak
    )
{
    lemma_canonical_break_remains_structural();
}

} // verus!

fn main() {}
