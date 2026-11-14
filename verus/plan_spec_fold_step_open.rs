//! The fold step with nothing buffered.
//!
//! With no block held, the planner's two buffering routes are unreachable: an
//! `Extend` needs a block to extend and a `Close` needs a delimiter to close one,
//! which is the fact [`lemma_no_block_cannot_buffer_or_close`] names. What is left
//! is the restart, which opens a block on the line, and the skip, which writes the
//! line through on its own.
//!
//! The two routes carry no released block, so the written half of the invariant
//! either stands still or gains one directly written line. What each route must
//! establish instead is the buffer half: the opening facts of a block one line old,
//! or the tracker being outside every fence after a direct write.

use crate::fence_spec::{spec_fence_next, spec_state_seeded};
use crate::production_fence::LineFeatures;
use crate::production_plan::{Agendum, Block as PlanBlock, Step};

use super::{
    ends_at, lemma_agreement_span_is_preserved, lemma_fold_preamble,
    lemma_no_block_cannot_buffer_or_close,
    lemma_not_fenced_means_unmasked, lemma_open_block_establishes,
    lemma_prefix_splits_at_seeded_state, lemma_restart_opens, lemma_skip_leaves_no_fence,
    lemma_verbatim_emission_agrees, lemma_written_extends, lemma_written_static, spec_absorb_all,
    spec_agendum_line, spec_balanced, spec_buffered, spec_drive, spec_emission_reads_back,
    spec_fold_buffered, spec_fold_chunk, spec_fold_facts, spec_fold_invariant, spec_fold_step,
    spec_fold_written, spec_fresh, spec_masked, spec_pending_lines, spec_plan_step,
    spec_same_reading,
    spec_step_agenda, spec_step_pending, spec_step_release, spec_step_start,
    spec_step_writes_line,
};

use vstd::prelude::*;

verus! {

/// One fold step with nothing buffered.
///
/// The dispatch is the same shape as the buffered one, and the two impossible
/// routes are discharged by the planner's own routing fact rather than by a case
/// analysis on the transition.
pub proof fn lemma_fold_step_open(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    plan_agenda: Seq<Agendum>,
    i: int,
)
    requires
        0 < i <= source.len(),
        i <= rewritable.len(),
        emitted.len() == source.len(),
        plan_agenda.len() == source.len(),
        spec_fold_invariant(source, emitted, rewritable, plan_agenda, i - 1),
        spec_drive(source, rewritable, i - 1).state.pending.is_none(),
        super::spec_extends(spec_drive(source, rewritable, i).out, plan_agenda),
        forall|j: int|
            #![trigger source[j]]
            0 <= j < source.len() ==> spec_emission_reads_back(
                plan_agenda[j],
                spec_agendum_line(source[j], #[trigger] emitted[j], plan_agenda[j]),
            ),
    ensures spec_fold_invariant(source, emitted, rewritable, plan_agenda, i),
{
    let step = spec_fold_step(source, rewritable, i);
    match step {
        Step::Restart(fresh) => lemma_fold_step_open_restart(
            source,
            emitted,
            rewritable,
            plan_agenda,
            i,
            fresh,
        ),
        Step::Skip => lemma_fold_step_open_skip(source, emitted, rewritable, plan_agenda, i),
        Step::Extend(_) => lemma_no_block_cannot_buffer_or_close(
            spec_drive(source, rewritable, i - 1).state.pending,
            spec_fold_facts(source, rewritable, i),
        ),
        Step::Close(_, _) => lemma_no_block_cannot_buffer_or_close(
            spec_drive(source, rewritable, i - 1).state.pending,
            spec_fold_facts(source, rewritable, i),
        ),
    }
}

/// The `Restart` route with nothing buffered: the line opens a block.
///
/// Nothing is written, so the written half carries over; the whole of the step is
/// the new block's opening facts and the empty conflict fold that the next step's
/// recurrence will extend.
pub proof fn lemma_fold_step_open_restart(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    plan_agenda: Seq<Agendum>,
    i: int,
    fresh: PlanBlock,
)
    requires
        0 < i <= source.len(),
        i <= rewritable.len(),
        emitted.len() == source.len(),
        plan_agenda.len() == source.len(),
        spec_fold_invariant(source, emitted, rewritable, plan_agenda, i - 1),
        spec_drive(source, rewritable, i - 1).state.pending.is_none(),
        super::spec_extends(spec_drive(source, rewritable, i).out, plan_agenda),
        spec_fold_step(source, rewritable, i) == Step::Restart(fresh),
        forall|j: int|
            #![trigger source[j]]
            0 <= j < source.len() ==> spec_emission_reads_back(
                plan_agenda[j],
                spec_agendum_line(source[j], #[trigger] emitted[j], plan_agenda[j]),
            ),
    ensures spec_fold_invariant(source, emitted, rewritable, plan_agenda, i),
{
    let prev = spec_drive(source, rewritable, i - 1);
    let next = spec_drive(source, rewritable, i);
    let line = source[i - 1];
    let before = spec_state_seeded(source, i - 1, None);
    let facts = spec_fold_facts(source, rewritable, i);
    let chunk = spec_fold_chunk(source, rewritable, i);
    lemma_fold_preamble(source, emitted, rewritable, plan_agenda, i);
    reveal(spec_fold_invariant);
    assert(spec_fold_buffered(source, rewritable, i - 1));
    reveal(spec_fold_buffered);
    assert(before.is_none());
    reveal(spec_fold_written);
    reveal(spec_balanced);
    reveal(spec_pending_lines);
    assert(prev.state.pending.is_none());
    assert(prev.out.len() == i - 1);
    reveal(spec_plan_step);
    assert(!facts.was_in_fence);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_step_writes_line);
    assert(chunk == Seq::<Agendum>::empty());
    assert(next.out.len() == prev.out.len());
    lemma_written_static(source, emitted, rewritable, plan_agenda, i);
    // The line opens a block of its own marker, run and depth.
    lemma_open_block_establishes(before, line, rewritable[i - 1], fresh);
    lemma_not_fenced_means_unmasked(before, line.depth);
    lemma_restart_opens(before, line, fresh.opening.marker);
    assert(spec_state_seeded(source, i, None) == spec_fence_next(before, line)) by {
        reveal_with_fuel(spec_state_seeded, 2);
    };
    assert(spec_state_seeded(source, i, None) == Some(fresh.opening));
    reveal(spec_masked);
    assert(spec_masked(before, fresh.opening.open_depth).is_none());
    // The buffer half, for a block one line old: an empty fold.
    reveal(spec_fold_buffered);
    reveal(spec_step_pending);
    reveal(spec_step_start);
    reveal(spec_buffered);
    reveal(spec_absorb_all);
    reveal(spec_fresh);
    assert(next.state.pending == Some(fresh));
    assert(next.state.start == i - 1);
    assert(spec_buffered(source, i, i) == Seq::<Option<LineFeatures>>::empty());
    assert(spec_absorb_all(
        spec_fresh(fresh.opening),
        Seq::<Option<LineFeatures>>::empty(),
        0,
    ) == spec_fresh(fresh.opening));
    assert forall|j: int|
        #![trigger spec_state_seeded(source, j, None)]
        i - 1 < j <= i implies spec_state_seeded(source, j, None) == Some(fresh.opening) by {
        assert(j == i);
    }
    assert(spec_fold_buffered(source, rewritable, i));
    reveal(spec_fold_invariant);
}

/// The `Skip` route with nothing buffered: the line is written through.
///
/// One span, one cut. The line is written verbatim, so the emission contract at its
/// position supplies the agreement the span needs, and the transition shows the
/// tracker leaves the line outside every fence — which is what the next direct
/// write requires.
pub proof fn lemma_fold_step_open_skip(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    plan_agenda: Seq<Agendum>,
    i: int,
)
    requires
        0 < i <= source.len(),
        i <= rewritable.len(),
        emitted.len() == source.len(),
        plan_agenda.len() == source.len(),
        spec_fold_invariant(source, emitted, rewritable, plan_agenda, i - 1),
        spec_drive(source, rewritable, i - 1).state.pending.is_none(),
        super::spec_extends(spec_drive(source, rewritable, i).out, plan_agenda),
        spec_fold_step(source, rewritable, i) == Step::Skip,
        forall|j: int|
            #![trigger source[j]]
            0 <= j < source.len() ==> spec_emission_reads_back(
                plan_agenda[j],
                spec_agendum_line(source[j], #[trigger] emitted[j], plan_agenda[j]),
            ),
    ensures spec_fold_invariant(source, emitted, rewritable, plan_agenda, i),
{
    let prev = spec_drive(source, rewritable, i - 1);
    let next = spec_drive(source, rewritable, i);
    let line = source[i - 1];
    let before = spec_state_seeded(source, i - 1, None);
    let facts = spec_fold_facts(source, rewritable, i);
    let chunk = spec_fold_chunk(source, rewritable, i);
    lemma_fold_preamble(source, emitted, rewritable, plan_agenda, i);
    reveal(spec_fold_invariant);
    assert(spec_fold_buffered(source, rewritable, i - 1));
    reveal(spec_fold_buffered);
    assert(before.is_none());
    reveal(spec_fold_written);
    reveal(spec_balanced);
    reveal(spec_pending_lines);
    assert(prev.state.pending.is_none());
    assert(prev.out.len() == i - 1);
    reveal(spec_plan_step);
    assert(!facts.was_in_fence);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_step_writes_line);
    assert(chunk.len() == 1);
    assert(chunk[0] == Agendum::Verbatim);
    assert(next.out.len() == i);
    assert(next.out.len() > prev.out.len());
    // The line's own span, from the emission contract at its position.
    assert(plan_agenda[i - 1] == chunk[0]);
    assert(spec_emission_reads_back(
        Agendum::Verbatim,
        spec_agendum_line(source[i - 1], emitted[i - 1], Agendum::Verbatim),
    ));
    lemma_verbatim_emission_agrees(source[i - 1], emitted[i - 1]);
    assert(spec_same_reading(source[i - 1], emitted[i - 1]));
    assert forall|j: int|
        #![trigger source[j]]
        i - 1 <= j < i implies spec_same_reading(source[j], emitted[j]) by {
        assert(j == i - 1);
    }
    lemma_agreement_span_is_preserved(source, emitted, i - 1, i, before);
    lemma_prefix_splits_at_seeded_state(source, i - 1, i, None);
    assert(ends_at(
        source,
        emitted,
        i - 1,
        i,
        spec_state_seeded(source, i - 1, None),
        spec_state_seeded(source, i, None),
    ));
    lemma_written_extends(source, emitted, rewritable, plan_agenda, i);
    // The tracker is outside and the line has no marker, so nothing opens.
    lemma_skip_leaves_no_fence(before, line);
    assert(spec_state_seeded(source, i, None) == spec_fence_next(before, line)) by {
        reveal_with_fuel(spec_state_seeded, 2);
    };
    assert(spec_state_seeded(source, i, None).is_none());
    reveal(spec_fold_buffered);
    reveal(spec_step_pending);
    assert(next.state.pending.is_none());
    assert(spec_fold_buffered(source, rewritable, i));
    reveal(spec_fold_invariant);
}

} // verus!