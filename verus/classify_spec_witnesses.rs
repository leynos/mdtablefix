//! Non-vacuity witnesses for the structural classifier, and the derivation
//! step they share.
//!
//! `verus/classify_spec.rs` states the character-sequence predicates the
//! production scanner reads, and `verus/lib.rs` composes them into
//! [`spec_classify`]. This file supplies the evidence that the composed
//! classifier is not vacuously constrained. Without it the classifier could
//! reject every line and the specification would still be true.
//!
//! Three kinds of item live here, and they sit together rather than beside the
//! definitions they mention because each is about `spec_classify` as a whole,
//! or about the literal break the formatter emits, and `verus/classify_spec.rs`
//! holds neither.
//!
//! The two indentation lemmas establish that a leading tab or four leading
//! spaces make a line literal — the branch that keeps indented content out of
//! structural classification.
//!
//! The three `..._exists` lemmas are the existence witnesses. One shows
//! ordinary digit-leading text classifies as paragraph text, one shows a
//! pipe-delimited dash row classifies as a table delimiter, and one shows the
//! formatter's own emitted break classifies as structural. Each reaches its
//! conclusion by unfolding the classifier's branches by hand, so they also
//! record what the classifier does at those three shapes.
//!
//! The `canonical_break` family then constrains that emitted line from four
//! directions at once: wrapping must observe it as a paragraph boundary, setext
//! conversion must reject it as candidate text, table buffering must reject it
//! as a structural table line, and orphan attachment must reject it before
//! specifier matching. Each of the four derives from
//! [`lemma_canonical_break_remains_structural`] rather than re-proving the
//! classification, so the four statements cannot disagree about the break.
//!
//! Extracted from `verus/lib.rs` to bring that file under the 400-line ceiling
//! `AGENTS.md` sets. The items are reached from `verus/lib.rs` through a
//! `#[path]`-declared child module rather than by name, so the mirror the
//! mutation gates build carries this file without a second list to keep in
//! step.

use vstd::prelude::*;

use crate::classify_spec::{
    spec_atx_heading, spec_body_starts_with_pipe, spec_contains, spec_fence_marker,
    spec_indentation_at, spec_is_blank_from, spec_is_markdown_whitespace, spec_line_parts,
    spec_line_parts_from, spec_list_item, spec_marker_count, spec_ordered_list_item,
    spec_setext_underline, spec_skip_pipes, spec_table_cell, spec_table_cells,
    spec_table_delimiter, spec_thematic_break, spec_thematic_valid, spec_trimmed_range,
};
use crate::production_classify::LineClass;

// `spec_classify` and its two inputs stay in the parent because the production
// modules reach them through the crate root, so they are named through `super`
// rather than through `classify_spec`, which does not define them.
use super::{canonical_break, canonical_context, spec_classify, ClassifyCtxView};

verus! {

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
    ensures
        spec_classify(canonical_break(), canonical_context()) == LineClass::ThematicBreak,
        forall|ctx: ClassifyCtxView| !ctx.is_in_fence ==>
            #[trigger] spec_classify(canonical_break(), ctx) == LineClass::ThematicBreak,
{
    let s = canonical_break();
    assert(s.len() == 70);
    assert(forall|i: int| 0 <= i < 70 ==> s[i] == '_');
    lemma_canonical_suffix(s, 0);
    assert(spec_line_parts(s) == (0int, false));
    assert(!spec_is_blank_from(s, 0));
    assert(!spec_contains(s, 0, 70, '|'));
    assert(!spec_table_delimiter(s, 0));
    assert(!spec_setext_underline(s, 0));
    assert(spec_thematic_break(s, 0));
    assert forall|ctx: ClassifyCtxView| !ctx.is_in_fence implies
        #[trigger] spec_classify(s, ctx) == LineClass::ThematicBreak by {
    }
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
    } == Some(LineClass::ThematicBreak),
    forall|ctx: ClassifyCtxView| !ctx.is_in_fence ==>
        #[trigger] spec_classify(canonical_break(), ctx) == LineClass::ThematicBreak,
{
    lemma_canonical_break_remains_structural();
}

/// Setext conversion cannot accept the canonical break as candidate text.
proof fn lemma_canonical_break_is_not_setext_text()
    ensures
        spec_classify(canonical_break(), canonical_context()) != LineClass::ParagraphText,
        forall|ctx: ClassifyCtxView| !ctx.is_in_fence ==>
            #[trigger] spec_classify(canonical_break(), ctx) != LineClass::ParagraphText,
{
    lemma_canonical_break_remains_structural();
}

/// Table buffering cannot accept the canonical break as a structural table line.
proof fn lemma_canonical_break_is_not_table_input()
    ensures
        spec_classify(canonical_break(), canonical_context()) != LineClass::TableDelimiter,
        spec_classify(canonical_break(), canonical_context()) != LineClass::TableRow,
        forall|ctx: ClassifyCtxView| !ctx.is_in_fence ==>
            #[trigger] spec_classify(canonical_break(), ctx) != LineClass::TableDelimiter
            && spec_classify(canonical_break(), ctx) != LineClass::TableRow,
{
    lemma_canonical_break_remains_structural();
}

/// Orphan attachment rejects the canonical break before matching specifier syntax.
proof fn lemma_canonical_break_is_not_orphan_specifier()
    ensures
        !(spec_classify(canonical_break(), canonical_context()) != LineClass::ThematicBreak),
        forall|ctx: ClassifyCtxView| !ctx.is_in_fence ==>
            #[trigger] spec_classify(canonical_break(), ctx) == LineClass::ThematicBreak,
{
    lemma_canonical_break_remains_structural();
}

} // verus!
