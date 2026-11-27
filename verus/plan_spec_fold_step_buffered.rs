//! The fold step for a block that stays open or ends at a delimiter.
//!
//! These are the two routes whose releasing line the tracker read from *inside* the
//! fence: `Extend`, which buffers the line into the block, and `Close`, which ends
//! the block and writes it with the closer attached. The other two routes — the
//! block abandoned because the tracker left the fence, or because a new one opens —
//! are in `plan_spec_fold_step_release.rs`, and the routes taken with nothing
//! buffered are in `plan_spec_fold_step_open.rs`.
//!
//! Both routes here are dischargeable from one conflict-fold step and one
//! transition lemma, which is the whole reason the invariant carries those two
//! facts rather than an accumulated verdict.

use crate::fence_spec::{spec_closes, spec_fence_next, spec_rewrite_permitted, spec_state_seeded};
use crate::production_fence::LineFeatures;
use crate::production_plan::{Agendum, Block as PlanBlock, Release, Step};

use super::{
    lemma_absorb_all_snoc, lemma_block_chunk_composes, lemma_buffered_context, lemma_close_step,
    lemma_extend_keeps, lemma_fold_preamble, lemma_interior_permitted, lemma_interior_shape,
    lemma_released_agenda_length, lemma_written_extends, lemma_written_static, spec_absorb,
    spec_absorb_all, spec_agendum_line, spec_balanced, spec_buffered, spec_drive,
    spec_emission_reads_back, spec_fold_buffered, spec_fold_chunk, spec_fold_facts,
    spec_fold_invariant, spec_fold_step, spec_fold_written, spec_fresh,
    spec_plan_step, spec_released_agenda, spec_step_agenda, spec_step_pending, spec_step_release,
    spec_step_start, spec_step_writes_line,
};

use vstd::prelude::*;

verus! {

/// One fold step with a block buffered.
pub proof fn lemma_fold_step_buffered(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    plan_agenda: Seq<Agendum>,
    i: int,
    block: PlanBlock,
)
    requires
        0 < i <= source.len(),
        i <= rewritable.len(),
        emitted.len() == source.len(),
        plan_agenda.len() == source.len(),
        spec_fold_invariant(source, emitted, rewritable, plan_agenda, i - 1),
        spec_drive(source, rewritable, i - 1).state.pending == Some(block),
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
        Step::Extend(extended) => {
            lemma_fold_step_extend(source, emitted, rewritable, plan_agenda, i, block, extended)
        },
        Step::Close(extended, release) => lemma_fold_step_close(
            source,
            emitted,
            rewritable,
            plan_agenda,
            i,
            block,
            extended,
            release,
        ),
        Step::Restart(fresh) => super::lemma_fold_step_abandon_restart(
            source,
            emitted,
            rewritable,
            plan_agenda,
            i,
            block,
            fresh,
        ),
        Step::Skip => super::lemma_fold_step_abandon_skip(
            source,
            emitted,
            rewritable,
            plan_agenda,
            i,
            block,
        ),
    }
}

/// The `Extend` route: the line is buffered and nothing is written.
///
/// The written half carries over unchanged, so the whole of the step is the buffer
/// half: the state recurrence gains the one point the invariant's quantifier will
/// range over next, and the conflict fold gains the one line it absorbed.
pub proof fn lemma_fold_step_extend(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    plan_agenda: Seq<Agendum>,
    i: int,
    block: PlanBlock,
    extended: PlanBlock,
)
    requires
        0 < i <= source.len(),
        i <= rewritable.len(),
        emitted.len() == source.len(),
        plan_agenda.len() == source.len(),
        spec_fold_invariant(source, emitted, rewritable, plan_agenda, i - 1),
        spec_drive(source, rewritable, i - 1).state.pending == Some(block),
        super::spec_extends(spec_drive(source, rewritable, i).out, plan_agenda),
        spec_fold_step(source, rewritable, i) == Step::Extend(extended),
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
    let start = prev.state.start;
    lemma_fold_preamble(source, emitted, rewritable, plan_agenda, i);
    lemma_buffered_context(source, rewritable, i, block);
    reveal(spec_fold_invariant);
    assert(spec_fold_buffered(source, rewritable, i - 1));
    assert(spec_fold_written(source, emitted, rewritable, plan_agenda, i - 1));
    reveal(spec_balanced);
    assert(prev.out.len() == start);
    reveal(spec_plan_step);
    reveal(spec_absorb);
    assert(facts.was_in_fence);
    assert(extended == spec_absorb(block, facts.features));
    assert(extended.opening == block.opening);
    // A buffered line writes nothing, so the span chain is untouched.
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_step_writes_line);
    assert(chunk == Seq::<Agendum>::empty());
    assert(next.out.len() == prev.out.len());
    lemma_written_static(source, emitted, rewritable, plan_agenda, i);
    // The tracker is still inside, so the transition keeps the state it was read
    // from, and the recurrence gains the point the next step will read.
    assert(spec_state_seeded(source, i, None) == spec_fence_next(before, line)) by {
        reveal_with_fuel(spec_state_seeded, 2);
    };
    assert(before == Some(block.opening));
    assert(facts.is_in_fence);
    lemma_extend_keeps(block.opening, line);
    assert(spec_fence_next(before, line) == Some(block.opening));
    assert(spec_state_seeded(source, i, None) == Some(extended.opening));
    lemma_absorb_all_snoc(block, source, start + 1, i);
    assert(extended.conflicts == spec_absorb_all(
        spec_fresh(extended.opening),
        spec_buffered(source, start + 1, i),
        i - start - 1,
    ).conflicts);
    // The buffer half, one line further on.
    reveal(spec_fold_buffered);
    reveal(spec_step_pending);
    reveal(spec_step_start);
    assert(next.state.pending == Some(extended));
    assert(next.state.start == start);
    assert forall|j: int|
        #![trigger spec_state_seeded(source, j, None)]
        start < j <= i implies spec_state_seeded(source, j, None) == Some(extended.opening) by {
        if j < i {
            assert(spec_state_seeded(source, j, None) == Some(block.opening));
        } else {
            assert(spec_state_seeded(source, i, None) == Some(extended.opening));
        }
    }
    assert(spec_fold_buffered(source, rewritable, i));
    reveal(spec_fold_invariant);
}

/// The `Close` route: the line ends the block and is written with it.
///
/// The released chunk is the block's whole agenda, so the span it preserves runs
/// from the opener to the closer. The two guarded-interior facts come from the
/// state recurrence and the conflict fold, and the closer relation comes from the
/// transition: a line that took the tracker out of the fence from inside it is that
/// fence's closer.
pub proof fn lemma_fold_step_close(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    plan_agenda: Seq<Agendum>,
    i: int,
    block: PlanBlock,
    extended: PlanBlock,
    release: Release,
)
    requires
        0 < i <= source.len(),
        i <= rewritable.len(),
        emitted.len() == source.len(),
        plan_agenda.len() == source.len(),
        spec_fold_invariant(source, emitted, rewritable, plan_agenda, i - 1),
        spec_drive(source, rewritable, i - 1).state.pending == Some(block),
        super::spec_extends(spec_drive(source, rewritable, i).out, plan_agenda),
        spec_fold_step(source, rewritable, i) == Step::Close(extended, release),
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
    let start = prev.state.start;
    let opener_rw = prev.state.opener_rewritable;
    lemma_fold_preamble(source, emitted, rewritable, plan_agenda, i);
    lemma_buffered_context(source, rewritable, i, block);
    reveal(spec_fold_invariant);
    assert(spec_fold_buffered(source, rewritable, i - 1));
    assert(spec_fold_written(source, emitted, rewritable, plan_agenda, i - 1));
    reveal(spec_balanced);
    assert(prev.out.len() == start);
    reveal(spec_plan_step);
    reveal(spec_absorb);
    // The closing branch is the only one that reports a closer, and it reports a
    // matched one.
    assert(release == Release::Matched);
    assert(facts.was_in_fence);
    assert(!facts.is_in_fence);
    assert(line.marker.is_some());
    assert(extended == spec_absorb(block, facts.features));
    assert(extended.opening == block.opening);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_step_writes_line);
    assert(chunk == spec_released_agenda(
        extended,
        i - start,
        opener_rw,
        facts.rewritable,
        release,
    ));
    lemma_released_agenda_length(extended, i - start, opener_rw, facts.rewritable, release);
    assert(chunk.len() == i - start);
    // The conflict fold now includes the closing line.
    lemma_absorb_all_snoc(block, source, start + 1, i);
    // Interior shape stops before the closer; rewrite permission covers it too.
    lemma_interior_shape(source, block.opening, start, i - 1);
    if !extended.conflicts {
        lemma_interior_permitted(source, extended, start, i);
        assert forall|j: int|
            #![trigger source[j]]
            start < j < i - 1 implies source[j].depth >= extended.opening.open_depth
                && !spec_closes(extended.opening, source[j])
                && spec_rewrite_permitted(extended.opening, source[j]) by {
        }
    }
    assert((!extended.conflicts && release == Release::Matched) ==> forall|j: int|
        #![trigger source[j]]
        start < j < i - 1 ==> source[j].depth >= extended.opening.open_depth
            && !spec_closes(extended.opening, source[j])
            && spec_rewrite_permitted(extended.opening, source[j]));
    assert((!extended.conflicts && release != Release::Matched) ==> forall|j: int|
        #![trigger source[j]]
        start < j < i ==> source[j].depth >= extended.opening.open_depth
            && !spec_closes(extended.opening, source[j])
            && spec_rewrite_permitted(extended.opening, source[j]));
    lemma_close_step(block.opening, line);
    assert(spec_closes(extended.opening, line));
    assert forall|j: int|
        #![trigger source[start + j]]
        0 <= j < i - start implies spec_emission_reads_back(
            chunk[j],
            spec_agendum_line(source[start + j], emitted[start + j], chunk[j]),
        ) by {
    }
    assert(spec_state_seeded(source, start, None) == spec_state_seeded(emitted, start, None));
    lemma_block_chunk_composes(
        source,
        emitted,
        chunk,
        start,
        i,
        extended,
        release,
        opener_rw,
        facts.rewritable,
    );
    // The written half: one span from the opener to the closer.
    assert(next.out.len() > prev.out.len());
    lemma_written_extends(source, emitted, rewritable, plan_agenda, i);
    // The buffer half: the close left nothing pending, and the tracker outside.
    assert(spec_state_seeded(source, i, None) == spec_fence_next(before, line)) by {
        reveal_with_fuel(spec_state_seeded, 2);
    };
    assert(before == Some(block.opening));
    assert(spec_state_seeded(source, i, None).is_none());
    reveal(spec_fold_buffered);
    reveal(spec_step_pending);
    assert(next.state.pending.is_none());
    assert(spec_fold_buffered(source, rewritable, i));
    reveal(spec_fold_invariant);
}

} // verus!