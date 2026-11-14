//! One induction step of the fold invariant: the vocabulary and the dispatch.
//!
//! The document theorem is an induction over the prefix, and each step is a case
//! split on what the planner decided about one line. This module names the three
//! quantities every route reads — the line's facts, the step chosen from them, and
//! the chunk the step writes — and proves the facts that hold whatever the route
//! was: the fold unfolded once, the plan correspondence projected onto the chunk,
//! and the emission contract transported to it.
//!
//! The routes themselves live in `plan_spec_fold_step_buffered.rs` and
//! `plan_spec_fold_step_open.rs`. They are separated by what the fold holds rather
//! than by size: with a block buffered every route is a statement about releasing
//! or extending it, and with nothing buffered the step can only be a direct write
//! or a fresh opening.

use crate::fence_spec::spec_state_seeded;
use crate::production_fence::LineFeatures;
use crate::production_plan::{Agendum, LineFacts, Step};

use super::{
    lemma_drive_length, lemma_extends_add_left, lemma_extends_index, lemma_extends_transitive,
    spec_agendum_line, spec_balanced, spec_drive, spec_drive_step, spec_emission_reads_back,
    spec_extends, spec_fold_invariant, spec_fold_written, spec_line_facts, spec_plan_step,
    spec_step_agenda,
};

use vstd::prelude::*;

verus! {

/// The facts the planner is handed for the line a step consumes.
pub open spec fn spec_fold_facts(
    source: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    i: int,
) -> LineFacts {
    spec_line_facts(spec_state_seeded(source, i - 1, None), source[i - 1], rewritable[i - 1])
}

/// The route the planner takes with the line at `i - 1`.
pub open spec fn spec_fold_step(source: Seq<LineFeatures>, rewritable: Seq<bool>, i: int) -> Step {
    spec_plan_step(spec_drive(source, rewritable, i - 1).state.pending, spec_fold_facts(
        source,
        rewritable,
        i,
    ))
}

/// The agenda one step contributes, as a function of the prefix alone.
///
/// The emitter appends this chunk to the document, so naming it here makes every
/// route's length and index arithmetic a statement about one term rather than
/// about a repeated application of the fold's step constructor.
pub open spec fn spec_fold_chunk(
    source: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    i: int,
) -> Seq<Agendum> {
    spec_step_agenda(spec_drive(source, rewritable, i - 1).state, spec_fold_facts(
        source,
        rewritable,
        i,
    ), i - 1)
}

/// One fold step, whatever is buffered.
///
/// The split is on the block the fold is holding, because that is the split the
/// planner's routing already makes.
pub proof fn lemma_fold_step(
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
        spec_extends(spec_drive(source, rewritable, i).out, plan_agenda),
        forall|j: int|
            #![trigger source[j]]
            0 <= j < source.len() ==> spec_emission_reads_back(
                plan_agenda[j],
                spec_agendum_line(source[j], #[trigger] emitted[j], plan_agenda[j]),
            ),
    ensures spec_fold_invariant(source, emitted, rewritable, plan_agenda, i),
{
    match spec_drive(source, rewritable, i - 1).state.pending {
        Some(block) => super::lemma_fold_step_buffered(
            source,
            emitted,
            rewritable,
            plan_agenda,
            i,
            block,
        ),
        None => super::lemma_fold_step_open(source, emitted, rewritable, plan_agenda, i),
    }
}

/// The facts every route needs, established once.
///
/// The fold unfolds at `i`, the shorter agenda is an initial segment of both the
/// longer one and the plan, the balance holds at the new prefix, and the chunk's
/// positions are the plan's own positions. The two quantifiers at the end are the
/// correspondence and the emission contract transported to the chunk, which is
/// what lets a route read an agendum without knowing where in the document the
/// fold currently is.
pub proof fn lemma_fold_preamble(
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
        spec_extends(spec_drive(source, rewritable, i).out, plan_agenda),
        forall|j: int|
            #![trigger source[j]]
            0 <= j < source.len() ==> spec_emission_reads_back(
                plan_agenda[j],
                spec_agendum_line(source[j], #[trigger] emitted[j], plan_agenda[j]),
            ),
    ensures
        spec_drive(source, rewritable, i) == spec_drive_step(
            spec_drive(source, rewritable, i - 1),
            spec_fold_facts(source, rewritable, i),
            i - 1,
        ),
        spec_extends(spec_drive(source, rewritable, i - 1).out, plan_agenda),
        spec_extends(
            spec_drive(source, rewritable, i - 1).out,
            spec_drive(source, rewritable, i).out,
        ),
        spec_balanced(spec_drive(source, rewritable, i), i),
        spec_fold_written(source, emitted, rewritable, plan_agenda, i - 1),
        spec_fold_facts(source, rewritable, i) == spec_line_facts(
            spec_state_seeded(source, i - 1, None),
            source[i - 1],
            rewritable[i - 1],
        ),
        spec_fold_step(source, rewritable, i) == spec_plan_step(
            spec_drive(source, rewritable, i - 1).state.pending,
            spec_fold_facts(source, rewritable, i),
        ),
        spec_fold_chunk(source, rewritable, i) == spec_step_agenda(
            spec_drive(source, rewritable, i - 1).state,
            spec_fold_facts(source, rewritable, i),
            i - 1,
        ),
        forall|k: int|
            #![trigger spec_fold_chunk(source, rewritable, i)[k]]
            0 <= k < spec_fold_chunk(source, rewritable, i).len() ==> plan_agenda[spec_drive(
                source,
                rewritable,
                i - 1,
            ).out.len() as int + k] == spec_fold_chunk(source, rewritable, i)[k],
        forall|k: int|
            #![trigger spec_fold_chunk(source, rewritable, i)[k]]
            0 <= k < spec_fold_chunk(source, rewritable, i).len() ==> spec_emission_reads_back(
                spec_fold_chunk(source, rewritable, i)[k],
                spec_agendum_line(
                    source[spec_drive(source, rewritable, i - 1).out.len() as int + k],
                    emitted[spec_drive(source, rewritable, i - 1).out.len() as int + k],
                    spec_fold_chunk(source, rewritable, i)[k],
                ),
            ),
{
    let prev = spec_drive(source, rewritable, i - 1);
    let next = spec_drive(source, rewritable, i);
    let facts = spec_fold_facts(source, rewritable, i);
    let chunk = spec_fold_chunk(source, rewritable, i);
    reveal(spec_fold_invariant);
    assert(spec_fold_written(source, emitted, rewritable, plan_agenda, i - 1));
    reveal(spec_drive);
    assert(next == spec_drive_step(prev, facts, i - 1)) by {
        reveal_with_fuel(spec_drive, 2);
    };
    reveal(spec_drive_step);
    assert(next.out == prev.out.add(chunk));
    reveal(spec_extends);
    lemma_extends_add_left(prev.out, chunk);
    lemma_extends_transitive(prev.out, next.out, plan_agenda);
    lemma_drive_length(source, rewritable, i);
    assert(spec_balanced(next, i));
    assert forall|k: int| #![trigger chunk[k]] 0 <= k < chunk.len() implies plan_agenda[
        prev.out.len() as int + k
    ] == chunk[k] by {
        lemma_extends_index(next.out, plan_agenda, prev.out.len() as int + k);
        vstd::seq::axiom_seq_add_index2(prev.out, chunk, prev.out.len() as int + k);
    };
    assert forall|k: int| #![trigger chunk[k]] 0 <= k < chunk.len() implies spec_emission_reads_back(
        chunk[k],
        spec_agendum_line(
            source[prev.out.len() as int + k],
            emitted[prev.out.len() as int + k],
            chunk[k],
        ),
    ) by {
        let j = prev.out.len() as int + k;
        assert(0 <= j < source.len());
        assert(spec_emission_reads_back(
            plan_agenda[j],
            spec_agendum_line(source[j], emitted[j], plan_agenda[j]),
        ));
        assert(plan_agenda[j] == chunk[k]);
    };
}

} // verus!