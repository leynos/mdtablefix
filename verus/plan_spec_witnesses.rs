//! Non-vacuity witnesses for the planner's document theorem.
//!
//! `verus/plan_spec.rs` states the document-level result; this file supplies the
//! evidence that its hypotheses are satisfiable and its conclusions have
//! content. Extracted so that the specification reads as a statement rather
//! than as a statement plus its evidence, and so that each file stays inside the
//! repository's 400-line ceiling (`AGENTS.md`).
//!
//! # Why the fold witnesses assert their unfoldings
//!
//! The three document-plan witnesses each end with an explicit `assert` naming
//! the fold's step at the last index, and then re-state the plan's fields
//! through a local binding. Those assertions are not redundant with the
//! `reveal` calls above them, and removing them breaks this file's verification
//! while the identical bodies verified when they lived in `plan_spec.rs`.
//!
//! The reason is that `spec_document_plan` is a composition of two functions
//! (`spec_finish` of `spec_drive`) whose unfolding is decided by the *solver's*
//! E-matching rather than by `reveal`. Across a module boundary `spec_drive`'s
//! recursive branch no longer unifies with the literal sequence the conclusion
//! names, so the solver stops one step short: it can establish the `unclosed`
//! and `unclosed_lines` fields, which the `if i <= 0` branch decides at once,
//! but not the `agenda`, which needs the recursion to close. Writing the final
//! step as an equality gives the solver that instance directly rather than
//! asking it to synthesize it, which is the same technique that makes
//! `verus/fence_spec_witnesses.rs` verify from a sibling module.
//!
//! A witness that quietly lost its conclusion to an unfolding gap would still
//! compile and would still be counted as verified evidence, so the assertions
//! are load-bearing rather than stylistic.

use crate::fence_spec::{
    spec_closes, spec_compressed, spec_fence_next, spec_fence_region, spec_regions_seeded,
    spec_state_seeded,
};
// Imported from the parent rather than through `crate`'s re-export of
// `plan_spec`, matching the sibling `verus/fence_spec_witnesses.rs`. The two
// routes are interchangeable here — both were measured — but naming the
// definition's own module keeps the dependency visible in the source rather
// than routed through `lib.rs`'s glob.
use super::{
    lemma_compressed_block_regions, lemma_document_plan_total, spec_absorb, spec_block_agenda,
    spec_block_agenda_seeded, spec_delimiter_agendum, spec_delimiters_eligible,
    spec_document_plan, spec_drive, spec_drive_step, spec_fenced, spec_finish,
    spec_is_outer_delimiter, spec_line_facts, spec_masked, spec_open_block, spec_plan_step,
    spec_planner_features, spec_released_agenda, spec_step_agenda, spec_step_opener_rewritable,
    spec_step_pending, spec_step_release, spec_step_start, spec_step_state,
};
use crate::production_fence::{FenceState, LineFeatures, Region};
use crate::production_plan::{Agendum, Block as PlanBlock, Release};
use vstd::prelude::*;

verus! {

// ---------------------------------------------------------------------------
// Non-vacuity witnesses
// ---------------------------------------------------------------------------

/// A prose line at the opening depth.
///
/// Built from `Seq::empty` and `push` rather than the `seq!` macro so its length
/// and contents follow from the sequence axioms directly, without the array view
/// machinery the macro introduces.
pub open spec fn spec_witness_prose() -> LineFeatures {
    LineFeatures { depth: 0, marker: None, marker_len: 0, trailing_blank: false }
}

/// The four-backtick opener used by the witnesses below.
pub open spec fn spec_witness_opener() -> FenceState {
    FenceState { marker: '`', marker_len: 4, open_depth: 0 }
}

/// A four-backtick delimiter at depth zero.
pub open spec fn spec_witness_bare_backticks(len: usize) -> LineFeatures {
    LineFeatures { depth: 0, marker: Some('`'), marker_len: len, trailing_blank: true }
}

/// The reading agreement lemma is satisfiable and reports a real region.
///
/// Without a witness the theorem could be true of no pair of lines at all and
/// the proof would be worth nothing. Two prose lines agree, and the lemma then
/// reports that both runs classify them alike — which for a seed outside any
/// fence is prose, a region the body pins rather than leaving free.
proof fn lemma_witness_agreement_is_live()
    ensures
        spec_regions_seeded(
            Seq::<LineFeatures>::empty().push(spec_witness_prose()),
            1,
            None,
        ) == spec_regions_seeded(
            Seq::<LineFeatures>::empty().push(spec_witness_prose()),
            1,
            None,
        ),
        spec_fence_region(None, spec_witness_prose()) == Region::Prose,
{
}

/// The compressed-block hypothesis set is satisfiable, and its conclusion is
/// a non-empty region sequence.
///
/// The block is the shortest one that exercises both halves of the argument: a
/// four-backtick opener, one prose line, and a four-backtick closer, compressed
/// to three backticks at each end. Its regions are the same three regions
/// either way, which is the theorem's conclusion made concrete rather than left
/// as an equality between two unknowns.
proof fn lemma_witness_compressed_block_is_live()
    ensures
        spec_regions_seeded(
            Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4)).push(
                spec_witness_prose(),
            ).push(spec_witness_bare_backticks(4)),
            3,
            None,
        ) == Seq::<Region>::empty().push(Region::Delim).push(Region::Literal).push(
            Region::Delim,
        ),
{
    let source = Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4)).push(
        spec_witness_prose(),
    ).push(spec_witness_bare_backticks(4));
    let emitted = Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(3)).push(
        spec_witness_prose(),
    ).push(spec_witness_bare_backticks(3));
    lemma_compressed_block_regions(None, source, emitted, 3, spec_witness_opener(), true);
    // Unfold both recursive specifications at the witness with enough fuel to
    // reach their zero-length base cases, then name the three regions one at a
    // time. Stating the sequence in full is what makes this a non-vacuity
    // witness: the theorem's conclusion is pinned to a value, not left as an
    // equality between two unknowns.
    reveal_with_fuel(spec_state_seeded, 8);
    reveal_with_fuel(spec_regions_seeded, 4);
    reveal(spec_fence_region);
    reveal(spec_fence_next);
    reveal(spec_masked);
    reveal(spec_closes);
    reveal(spec_compressed);
    assert(spec_state_seeded(source, 0, None).is_none());
    assert(spec_fence_region(None, source[0]) == Region::Delim);
    assert(spec_regions_seeded(source, 1, None) == Seq::<Region>::empty().push(Region::Delim));
    assert(spec_fence_region(Some(spec_witness_opener()), source[1]) == Region::Literal);
    assert(spec_regions_seeded(source, 2, None) == Seq::<Region>::empty().push(Region::Delim).push(
        Region::Literal,
    ));
    assert(spec_closes(spec_witness_opener(), source[2]));
    assert(spec_fence_region(Some(spec_witness_opener()), source[2]) == Region::Delim);
    assert(spec_regions_seeded(source, 3, None) == Seq::<Region>::empty().push(Region::Delim).push(
        Region::Literal,
    ).push(Region::Delim));
}

/// `LEM-COMPRESSION-CONFLICT-IS-LOAD-BEARING`.
///
/// The block hypothesis is not vacuous and not a convenience: drop it and the
/// conclusion is false. An interior three-backtick line inside a four-backtick
/// block is literal content under the source opener but a closing delimiter under
/// the compressed one, so the line after it leaves the literal region. This is
/// issue #480 stated as a pair of region values, and it is why the planner keeps
/// a conflict flag at all.
proof fn lemma_witness_conflict_moves_region()
    ensures
        !spec_closes(spec_witness_opener(), spec_witness_bare_backticks(3)),
        spec_fence_region(
            Some(spec_witness_opener()),
            spec_witness_bare_backticks(3),
        ) == Region::Literal,
        spec_fence_region(
            Some(spec_compressed(spec_witness_opener())),
            spec_witness_bare_backticks(3),
        ) == Region::Delim,
        spec_fence_next(
            Some(spec_compressed(spec_witness_opener())),
            spec_witness_bare_backticks(3),
        ).is_none(),
{
}

/// `LEM-WITNESS-MARKER-FAMILY-IS-LOAD-BEARING`.
///
/// A tilde line against a backtick opener is the shape the fence mutation gate
/// drops, and this witness pins what the correct rule says about it: the line
/// does not close the block, so it stays literal, and the opener is still open
/// afterwards. The whole region conclusion would flip if the marker comparison
/// were removed, which is why the mutation is detectable at all.
proof fn lemma_witness_marker_family_holds_the_region()
    ensures
        !spec_closes(spec_witness_opener(), spec_witness_tildes(4)),
        spec_fence_region(Some(spec_witness_opener()), spec_witness_tildes(4)) == Region::Literal,
        spec_fence_next(Some(spec_witness_opener()), spec_witness_tildes(4))
            == Some(spec_witness_opener()),
{
}

/// A four-tilde delimiter at depth zero.
///
/// The marker family the backtick opener above must refuse, so the two
/// witnesses together exercise the closing rule's comparison in the direction
/// the compression pass relies on.
pub open spec fn spec_witness_tildes(len: usize) -> LineFeatures {
    LineFeatures { depth: 0, marker: Some('~'), marker_len: len, trailing_blank: true }
}

/// `LEM-WITNESS-DEPTH-DROP-RELEASES-THE-BLOCK`.
///
/// A line shallower than the opener cannot be content inside it: `CommonMark`
/// ends the block when the blockquote prefix shortens, and the transition
/// checks the depth before it reads the line. The witness pins the three
/// consequences together — the block is gone, the line is prose rather than
/// literal, and it opens a new block of its own depth — which is also why the
/// interior lemma can exclude these lines by hypothesis without losing any
/// line the tracker would actually read as interior.
proof fn lemma_witness_depth_drop_releases_the_block()
    ensures
        spec_fence_next(Some(spec_witness_depth_drop()), spec_witness_shallow_marker())
            == Some(
                FenceState { marker: '`', marker_len: 3, open_depth: 0 },
            ),
        spec_fence_region(Some(spec_witness_depth_drop()), spec_witness_shallow_marker())
            == Region::Delim,
        !spec_fenced(Some(spec_witness_depth_drop()), spec_witness_shallow_marker().depth),
{
}

/// A backtick opener at depth one.
///
/// Paired with the depth-zero marker line below, it exhibits the depth drop that
/// ends the block before the line is read.
pub open spec fn spec_witness_depth_drop() -> FenceState {
    FenceState { marker: '`', marker_len: 3, open_depth: 1 }
}

/// A three-backtick marker at depth zero — the line that drops the blockquote.
pub open spec fn spec_witness_shallow_marker() -> LineFeatures {
    LineFeatures { depth: 0, marker: Some('`'), marker_len: 3, trailing_blank: true }
}

/// `LEM-WITNESS-INCOMPATIBLE-ENDPOINTS-FORBID-REWRITE`.
///
/// An opener that may be respelled together with a closer that may not is the
/// pair `delimiters_eligible` must refuse, and the witness pins the refusal:
/// rewriting only the opener would leave a backtick block facing a tilde closer
/// that no longer closes it. This is the shape that makes the eligibility rule
/// load-bearing rather than a formality, and it is the mixed-family case the
/// fixture corpus does not otherwise reach.
proof fn lemma_witness_incompatible_endpoints_forbid_rewrite()
    ensures
        !spec_delimiters_eligible(Release::Matched, true, false),
        !spec_delimiters_eligible(Release::Matched, false, true),
        spec_delimiters_eligible(Release::Matched, true, true),
        !spec_delimiters_eligible(Release::Abandoned, true, true),
{
}

// ---------------------------------------------------------------------------
// Witnesses over the fold
// ---------------------------------------------------------------------------

/// The fold's own decision, pinned as values rather than as an equation.
///
/// The witnesses above are all about the *rules* — a transition, a guard, an
/// eligibility predicate. None of them exercises [`spec_drive`], so nothing in
/// the block above would fail if the fold wired those rules together wrongly.
/// The four below close that gap by naming the fold's whole output for the four
/// routes a document can take, which is the smallest evidence that the
/// composition's subject is non-empty: a document the fold drives to a close, a
/// document that ends inside a block, a document the fold abandons at a depth
/// change, and the empty document.
///
/// Each is stated as an equality between the plan and a literal sequence, not as
/// an equality between two unknowns, so the witness fails if *either* side moves.

/// The document the fold drives to a matched close.
///
/// The shortest matched block there is: a four-backtick opener and its own close.
/// The whole plan is pinned, so a fold that mis-routed the closer, mis-counted
/// the span, or lost a line would change one of these three values. The agenda is
/// all `Compressed` and not `Normalized` because the block has no interior line at
/// all, so its conflict flag is still false when it is released and the pass takes
/// the compression route — which is what makes this witness the compressed half's
/// concrete instance rather than a second copy of the agreement case.
proof fn lemma_witness_matched_block_plan_is_live()
    ensures
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_bare_backticks(4))
                .push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true).push(true),
        ).agenda == Seq::<Agendum>::empty()
            .push(Agendum::Compressed)
            .push(Agendum::Compressed),
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_bare_backticks(4))
                .push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true).push(true),
        ).unclosed.is_none(),
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_bare_backticks(4))
                .push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true).push(true),
        ).unclosed_lines == 0,
{
    let source = Seq::<LineFeatures>::empty()
        .push(spec_witness_bare_backticks(4))
        .push(spec_witness_bare_backticks(4));
    let rewritable = Seq::<bool>::empty().push(true).push(true);
    reveal_with_fuel(spec_drive, 6);
    reveal_with_fuel(spec_block_agenda_seeded, 8);
    reveal_with_fuel(spec_state_seeded, 8);
    reveal(spec_document_plan);
    reveal(spec_finish);
    reveal(spec_drive_step);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_released_agenda);
    reveal(spec_block_agenda);
    reveal(spec_delimiter_agendum);
    reveal(spec_is_outer_delimiter);
    reveal(spec_delimiters_eligible);
    reveal(spec_plan_step);
    reveal(spec_absorb);
    reveal(spec_open_block);
    reveal(spec_step_state);
    reveal(spec_step_pending);
    reveal(spec_line_facts);
    reveal(spec_fenced);
    reveal(spec_planner_features);
    reveal(spec_closes);
    reveal(spec_fence_next);
    // The fold's own step at the final index; see the module comment.
    assert(spec_drive(source, rewritable, 2) == spec_drive_step(
        spec_drive(source, rewritable, 1),
        spec_line_facts(spec_state_seeded(source, 1, None), source[1], rewritable[1]),
        1,
    ));
    let d = spec_drive(source, rewritable, 2);
    assert(spec_finish(d, 2).agenda == Seq::<Agendum>::empty()
        .push(Agendum::Compressed)
        .push(Agendum::Compressed));
    assert(spec_finish(d, 2).unclosed.is_none());
    assert(spec_finish(d, 2).unclosed_lines == 0);
}

/// The document the fold leaves inside an unmatched block.
///
/// This is the fourth flush path, and it is the one the corpus cannot reach:
/// every fixture under `tests/data/` closes its fences, so no fixture exercises
/// `Release::Unmatched`. The witness is the shortest document that does — a lone
/// opener — and it pins all three of the plan's fields, including the diagnostic
/// the adapter reports, so a fold that failed to record the unclosed block would
/// fail here rather than only in the corpus sweep whose documents supply one by
/// hand.
proof fn lemma_witness_unmatched_block_plan_is_live()
    ensures
        spec_document_plan(
            Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true),
        ).agenda == Seq::<Agendum>::empty().push(Agendum::Compressed),
        spec_document_plan(
            Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true),
        ).unclosed_lines == 1,
        spec_document_plan(
            Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true),
        ).unclosed == Some(
            PlanBlock {
                opening: FenceState { marker: '`', marker_len: 4, open_depth: 0 },
                conflicts: false,
            },
        ),
{
    let source = Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4));
    let rewritable = Seq::<bool>::empty().push(true);
    reveal_with_fuel(spec_drive, 6);
    reveal_with_fuel(spec_block_agenda_seeded, 8);
    reveal_with_fuel(spec_state_seeded, 8);
    reveal(spec_document_plan);
    reveal(spec_finish);
    reveal(spec_drive_step);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_released_agenda);
    reveal(spec_block_agenda);
    reveal(spec_delimiter_agendum);
    reveal(spec_is_outer_delimiter);
    reveal(spec_delimiters_eligible);
    reveal(spec_plan_step);
    reveal(spec_absorb);
    reveal(spec_open_block);
    reveal(spec_step_state);
    reveal(spec_step_pending);
    reveal(spec_step_start);
    reveal(spec_step_opener_rewritable);
    reveal(spec_line_facts);
    reveal(spec_fenced);
    reveal(spec_planner_features);
    reveal(spec_closes);
    reveal(spec_fence_next);
    // The fold's own step at the final index; see the module comment.
    assert(spec_drive(source, rewritable, 1) == spec_drive_step(
        spec_drive(source, rewritable, 0),
        spec_line_facts(spec_state_seeded(source, 0, None), source[0], rewritable[0]),
        0,
    ));
    let d1 = spec_drive(source, rewritable, 1);
    assert(d1.out == Seq::<Agendum>::empty());
    assert(d1.state.pending.is_some());
    assert(spec_finish(d1, 1).agenda == Seq::<Agendum>::empty().push(Agendum::Compressed));
}

/// The document the fold drives to the conflict fallback.
///
/// A four-backtick opener, an interior three-backtick line, and a payload line.
/// The interior line is the issue #480 shape: under the source opener it is
/// literal content, and under the compressed opener the pass would write it is a
/// closing delimiter, so compressing the block would move the payload line out of
/// the literal region. The planner therefore records a conflict and the release
/// writes `Normalized` rather than `Compressed` — and the two later lines are
/// `Verbatim`, because the release covers them without rewriting either.
///
/// The three agenda together are the fallback's whole observable effect: the
/// opener keeps its source spelling, and the two lines that follow it are written
/// byte for byte. A planner whose conflict fold missed this interior line would
/// emit `Compressed` here, which is exactly the defect the compression mutation
/// gate injects.
proof fn lemma_witness_conflict_fallback_plan_is_live()
    ensures
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_bare_backticks(4))
                .push(spec_witness_bare_backticks(3))
                .push(spec_witness_prose()),
            Seq::<bool>::empty().push(true).push(true).push(true),
        ).agenda == Seq::<Agendum>::empty()
            .push(Agendum::Normalized)
            .push(Agendum::Verbatim)
            .push(Agendum::Verbatim),
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_bare_backticks(4))
                .push(spec_witness_bare_backticks(3))
                .push(spec_witness_prose()),
            Seq::<bool>::empty().push(true).push(true).push(true),
        ).unclosed_lines == 3,
{
    let source = Seq::<LineFeatures>::empty()
        .push(spec_witness_bare_backticks(4))
        .push(spec_witness_bare_backticks(3))
        .push(spec_witness_prose());
    let rewritable = Seq::<bool>::empty().push(true).push(true).push(true);
    reveal_with_fuel(spec_drive, 5);
    reveal_with_fuel(spec_block_agenda_seeded, 5);
    reveal_with_fuel(spec_state_seeded, 5);
    reveal(spec_document_plan);
    reveal(spec_finish);
    reveal(spec_drive_step);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_released_agenda);
    reveal(spec_block_agenda);
    reveal(spec_delimiter_agendum);
    reveal(spec_is_outer_delimiter);
    reveal(spec_delimiters_eligible);
    reveal(spec_plan_step);
    reveal(spec_absorb);
    reveal(spec_open_block);
    reveal(spec_step_state);
    reveal(spec_step_pending);
    reveal(spec_line_facts);
    reveal(spec_fenced);
    reveal(spec_planner_features);
    reveal(spec_closes);
    reveal(spec_fence_next);
    // The fold's own step at the final index; see the module comment.
    assert(spec_drive(source, rewritable, 3) == spec_drive_step(
        spec_drive(source, rewritable, 2),
        spec_line_facts(spec_state_seeded(source, 2, None), source[2], rewritable[2]),
        2,
    ));
    let d = spec_drive(source, rewritable, 3);
    assert(spec_finish(d, 3).agenda == Seq::<Agendum>::empty()
        .push(Agendum::Normalized)
        .push(Agendum::Verbatim)
        .push(Agendum::Verbatim));
    assert(spec_finish(d, 3).unclosed_lines == 3);
}

/// The empty document's plan, which is the fold's base case.
///
/// The base case is not decoration: it is the document consisting of no lines,
/// it is one of the inputs the length theorem claims, and it is the only input on
/// which the fold's own definition is reached rather than its step. Pinning it as
/// a value keeps the fold's totality from resting on an unexamined arm — a plan
/// with no agenda, no unclosed block, and no reported lines.
proof fn lemma_witness_empty_document_plan_is_live()
    ensures
        spec_document_plan(Seq::<LineFeatures>::empty(), Seq::<bool>::empty()).agenda
            == Seq::<Agendum>::empty(),
        spec_document_plan(Seq::<LineFeatures>::empty(), Seq::<bool>::empty()).unclosed.is_none(),
        spec_document_plan(Seq::<LineFeatures>::empty(), Seq::<bool>::empty()).unclosed_lines == 0,
{
    lemma_document_plan_total(Seq::<LineFeatures>::empty(), Seq::<bool>::empty());
    reveal(spec_document_plan);
    reveal(spec_drive);
    reveal(spec_finish);
}

} // verus!
