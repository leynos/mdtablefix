//! Non-vacuity witnesses for the planner's document-level fold.
//!
//! The witnesses here answer the document theorem's existential question: is
//! there any document at all that satisfies its hypotheses, and does the plan it
//! produces have the content the theorem claims? Without them the theorem could
//! hold over a hypothesis nothing satisfies and be worth nothing.
//!
//! They live apart from the block-level witnesses in
//! `verus/plan_spec_witnesses.rs` because they need a different fixture: each one
//! runs `spec_document_plan` over a whole source sequence rather than over a
//! single line pair, so each builds its `Seq<LineFeatures>` and `Seq<bool>` by
//! hand and reads the resulting agenda, unclosed marker and line count.
//!
//! # Why these assert their unfoldings
//!
//! The three fold-level witnesses each end with an explicit `assert` naming the
//! fold's step at the last index, and then re-state the plan's fields through a
//! local binding. Those assertions are not redundant with the `reveal` calls
//! above them: removing them breaks this file's verification, while the
//! identical bodies verified when they lived in `plan_spec.rs`.
//!
//! The reason is that `spec_document_plan` is a composition of two functions
//! (`spec_finish` of `spec_drive`) whose unfolding is decided by the *solver's*
//! E-matching rather than by `reveal`. Across a module boundary `spec_drive`'s
//! recursive branch no longer unifies with the literal sequence the conclusion
//! names, so the solver stops one step short: it can establish the `unclosed`
//! and `unclosed_lines` fields, which the `if i <= 0` branch decides at once,
//! but not the `agenda`, which needs the recursion to close. Writing the final
//! step as an equality gives the solver that instance directly rather than
//! asking it to synthesize it.
//!
//! A witness that quietly lost its conclusion to an unfolding gap would still
//! compile and would still be counted as verified evidence, so the assertions
//! are load-bearing rather than stylistic.
//!
//! Extracted from `verus/plan_spec_witnesses.rs` so both files come in under the
//! 400-line ceiling `AGENTS.md` sets.

use crate::fence_spec::{spec_closes, spec_fence_next, spec_state_seeded};
// The planner specifications are reached through `plan_spec`'s re-export at the
// crate root, which is the route `verus/lib.rs` publishes for them; naming the
// module here instead would make this file depend on the specification's own
// module path rather than on the surface the proof entry point exposes.
use crate::{
    lemma_document_plan_total, spec_absorb, spec_block_agenda, spec_block_agenda_seeded,
    spec_delimiter_agendum, spec_delimiters_eligible, spec_document_plan, spec_drive,
    spec_drive_step, spec_fenced, spec_finish, spec_is_outer_delimiter, spec_line_facts,
    spec_open_block, spec_plan_step, spec_planner_features, spec_released_agenda,
    spec_step_agenda, spec_step_opener_rewritable, spec_step_pending, spec_step_release,
    spec_step_start, spec_step_state,
};
use super::{spec_witness_bare_backticks, spec_witness_prose};
use crate::production_fence::{FenceState, LineFeatures};
use crate::production_plan::{Agendum, Block as PlanBlock};
use vstd::prelude::*;

verus! {


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
