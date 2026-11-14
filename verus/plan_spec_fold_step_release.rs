//! The fold step for a block the tracker abandoned.
//!
//! The two routes here are the ones whose releasing line the tracker read from
//! *outside* the fence: `Restart`, which ends the block and opens a new one on the
//! same line, and `Skip`, which ends the block and writes the line on its own. The
//! block is released as abandoned, which is why its delimiters are never respelled
//! and its span owes no guarded-interior facts beyond the ones the invariant
//! already carries.
//!
//! A skip is the one route that writes two spans in a single step — the released
//! block and then the line — so its chain step is the join lemma rather than a
//! single span. That is the only structural difference between it and a restart.

use crate::fence_spec::{spec_closes, spec_fence_next, spec_rewrite_permitted, spec_state_seeded};
use crate::production_fence::LineFeatures;
use crate::production_plan::{Agendum, Block as PlanBlock, Release, Step};

use super::{
    ends_at, lemma_agreement_span_is_preserved, lemma_block_chunk_composes, lemma_buffered_context,
    lemma_ends_at_components, lemma_ends_at_join, lemma_fold_preamble, lemma_interior_permitted,
    lemma_interior_shape,
    lemma_not_fenced_means_unmasked, lemma_open_block_establishes,
    lemma_prefix_splits_at_seeded_state, lemma_released_agenda_length, lemma_restart_opens,
    lemma_skip_leaves_no_fence, lemma_skip_outside_is_unmarked, lemma_verbatim_emission_agrees,
    lemma_written_extends, spec_absorb_all, spec_agendum_line, spec_balanced, spec_buffered,
    spec_drive, spec_emission_reads_back, spec_fold_buffered, spec_fold_chunk, spec_fold_facts,
    spec_fold_invariant, spec_fold_step, spec_fold_written, spec_fresh, spec_masked,
    spec_plan_step, spec_released_agenda, spec_same_reading, spec_step_agenda, spec_step_pending,
    spec_step_release, spec_step_start, spec_step_writes_line,
};

use vstd::prelude::*;

verus! {

/// The span an abandoned block preserves, from its opener to the line before the
/// one that ended it.
///
/// Both abandoned routes release exactly this span, so both prove it here and only
/// have to describe what they do with the current line afterwards. The block's
/// delimiters are not respelled — an abandoned release is never eligible — so the
/// span is pointwise agreement over the block plus the opener's read-back, and the
/// guarded-interior facts it needs come from the invariant rather than from any
/// new case analysis.
pub proof fn lemma_abandoned_span(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    chunk: Seq<Agendum>,
    start: int,
    i: int,
    block: PlanBlock,
    opener_rw: bool,
)
    requires
        emitted.len() == source.len(),
        0 <= start < i - 1 <= source.len(),
        chunk == spec_released_agenda(block, i - 1 - start, opener_rw, false, Release::Abandoned),
        // The chunk's length is the released span's, which is what puts the
        // contract's index inside the document.
        chunk.len() == i - 1 - start,
        forall|k: int|
            #![trigger chunk[k]]
            0 <= k < chunk.len() ==> spec_emission_reads_back(
                chunk[k],
                spec_agendum_line(source[start + k], emitted[start + k], chunk[k]),
            ),
        source[start].marker == Some(block.opening.marker),
        source[start].marker_len == block.opening.marker_len,
        source[start].depth == block.opening.open_depth,
        spec_masked(spec_state_seeded(source, start, None), block.opening.open_depth).is_none(),
        spec_state_seeded(source, start, None) == spec_state_seeded(emitted, start, None),
        forall|j: int|
            #![trigger spec_state_seeded(source, j, None)]
            start < j <= i - 1 ==> spec_state_seeded(source, j, None) == Some(block.opening),
        block.conflicts == spec_absorb_all(
            spec_fresh(block.opening),
            spec_buffered(source, start + 1, i - 1),
            i - 1 - start - 1,
        ).conflicts,
    ensures
        ends_at(
            source,
            emitted,
            start,
            i - 1,
            spec_state_seeded(source, start, None),
            spec_state_seeded(source, i - 1, None),
        ),
        spec_state_seeded(source, i - 1, None) == spec_state_seeded(emitted, i - 1, None),
{
    lemma_interior_shape(source, block.opening, start, i - 1);
    if !block.conflicts {
        lemma_interior_permitted(source, block, start, i - 1);
        assert forall|j: int|
            #![trigger source[j]]
            start < j < i - 1 implies source[j].depth >= block.opening.open_depth
                && !spec_closes(block.opening, source[j])
                && spec_rewrite_permitted(block.opening, source[j]) by {
        }
    }
    assert((!block.conflicts && Release::Abandoned != Release::Matched) ==> forall|j: int|
        #![trigger source[j]]
        start < j < i - 1 ==> source[j].depth >= block.opening.open_depth
            && !spec_closes(block.opening, source[j])
            && spec_rewrite_permitted(block.opening, source[j]));
    // The released-chunk lemma indexes its contract by document position; the
    // released agenda's length is what makes the two indexings the same range.
    lemma_released_agenda_length(block, i - 1 - start, opener_rw, false, Release::Abandoned);
    assert(chunk.len() == i - 1 - start);
    assert forall|j: int|
        #![trigger source[start + j]]
        0 <= j < i - 1 - start implies spec_emission_reads_back(
            chunk[j],
            spec_agendum_line(source[start + j], emitted[start + j], chunk[j]),
        ) by {
    }
    lemma_block_chunk_composes(
        source,
        emitted,
        chunk,
        start,
        i - 1,
        block,
        Release::Abandoned,
        opener_rw,
        false,
    );
}

/// The `Restart` route: the block is released and a new one opens on the line.
///
/// The line is outside the fence, so it opens the block its marker names and the
/// buffer half becomes a statement about that fresh block. Its conflict fold is the
/// empty fold, because the block has absorbed nothing but its own opener, which is
/// the base case the next step's recurrence extends.
pub proof fn lemma_fold_step_abandon_restart(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    plan_agenda: Seq<Agendum>,
    i: int,
    block: PlanBlock,
    fresh: PlanBlock,
)
    requires
        0 < i <= source.len(),
        i <= rewritable.len(),
        emitted.len() == source.len(),
        plan_agenda.len() == source.len(),
        spec_fold_invariant(source, emitted, rewritable, plan_agenda, i - 1),
        spec_drive(source, rewritable, i - 1).state.pending == Some(block),
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
    let start = prev.state.start;
    let opener_rw = prev.state.opener_rewritable;
    lemma_fold_preamble(source, emitted, rewritable, plan_agenda, i);
    lemma_buffered_context(source, rewritable, i, block);
    reveal(spec_fold_invariant);
    assert(spec_fold_buffered(source, rewritable, i - 1));
    reveal(spec_fold_written);
    reveal(spec_balanced);
    assert(prev.out.len() == start);
    reveal(spec_plan_step);
    assert(!facts.was_in_fence);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_step_writes_line);
    assert(chunk == spec_released_agenda(
        block,
        i - 1 - start,
        opener_rw,
        false,
        Release::Abandoned,
    ));
    lemma_released_agenda_length(block, i - 1 - start, opener_rw, false, Release::Abandoned);
    assert(chunk.len() == i - 1 - start);
    assert(spec_state_seeded(source, start, None) == spec_state_seeded(emitted, start, None));
    assert forall|k: int|
        #![trigger chunk[k]]
        0 <= k < chunk.len() implies spec_emission_reads_back(
            chunk[k],
            spec_agendum_line(source[start + k], emitted[start + k], chunk[k]),
        ) by {
    }
    lemma_abandoned_span(source, emitted, chunk, start, i, block, opener_rw);
    // The written half gains the abandoned block's span.
    assert(next.out.len() == i - 1);
    assert(next.out.len() > prev.out.len());
    lemma_written_extends(source, emitted, rewritable, plan_agenda, i);
    // The fresh block's opening facts come from the transition.
    lemma_open_block_establishes(before, line, rewritable[i - 1], fresh);
    lemma_not_fenced_means_unmasked(before, line.depth);
    lemma_restart_opens(before, line, fresh.opening.marker);
    assert(spec_state_seeded(source, i, None) == spec_fence_next(before, line)) by {
        reveal_with_fuel(spec_state_seeded, 2);
    };
    assert(spec_state_seeded(source, i, None) == Some(fresh.opening));
    // The buffer half, for a block one line old.
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

/// The `Skip` route: the block is released and the line is written on its own.
///
/// The step writes two spans, so the chain gains one cut and the composition gets
/// their join. The line itself is written verbatim, which is why the emission
/// contract at its position is what supplies the agreement the second span needs.
pub proof fn lemma_fold_step_abandon_skip(
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
    let start = prev.state.start;
    let opener_rw = prev.state.opener_rewritable;
    let released = spec_released_agenda(block, i - 1 - start, opener_rw, false, Release::Abandoned);
    lemma_fold_preamble(source, emitted, rewritable, plan_agenda, i);
    lemma_buffered_context(source, rewritable, i, block);
    reveal(spec_fold_invariant);
    assert(spec_fold_buffered(source, rewritable, i - 1));
    reveal(spec_fold_written);
    reveal(spec_balanced);
    assert(prev.out.len() == start);
    reveal(spec_plan_step);
    assert(!facts.was_in_fence);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_step_writes_line);
    assert(chunk == released.push(Agendum::Verbatim));
    lemma_released_agenda_length(block, i - 1 - start, opener_rw, false, Release::Abandoned);
    vstd::seq::axiom_seq_push_len(released, Agendum::Verbatim);
    assert(released.len() == i - 1 - start);
    assert(chunk.len() == i - start);
    assert(spec_state_seeded(source, start, None) == spec_state_seeded(emitted, start, None));
    // The abandoned block's own span. The chunk the step writes is the released
    // agenda and then the line, so at the released agenda's positions the chunk
    // and the released agenda name the same entry; the preamble's contract is
    // stated over the chunk.
    assert forall|k: int|
        #![trigger released[k]]
        0 <= k < released.len() implies spec_emission_reads_back(
            released[k],
            spec_agendum_line(source[start + k], emitted[start + k], released[k]),
        ) by {
        vstd::seq::axiom_seq_push_index_different(released, Agendum::Verbatim, k);
        assert(chunk[k] == released[k]);
        assert(spec_emission_reads_back(
            chunk[k],
            spec_agendum_line(source[start + k], emitted[start + k], chunk[k]),
        ));
        assert(plan_agenda[start + k] == chunk[k]);
    }
    lemma_abandoned_span(source, emitted, released, start, i, block, opener_rw);
    // The line the release follows is written verbatim.
    assert(plan_agenda[i - 1] == chunk[i - 1 - start]);
    vstd::seq::axiom_seq_push_index_same(released, Agendum::Verbatim, i - 1 - start);
    assert(chunk[i - 1 - start] == Agendum::Verbatim);
    assert(spec_emission_reads_back(
        Agendum::Verbatim,
        spec_agendum_line(source[i - 1], emitted[i - 1], Agendum::Verbatim),
    ));
    lemma_verbatim_emission_agrees(source[i - 1], emitted[i - 1]);
    assert forall|j: int|
        #![trigger source[j]]
        i - 1 <= j < i implies spec_same_reading(source[j], emitted[j]) by {
        assert(j == i - 1);
    }
    lemma_agreement_span_is_preserved(
        source,
        emitted,
        i - 1,
        i,
        spec_state_seeded(source, i - 1, None),
    );
    lemma_prefix_splits_at_seeded_state(source, i - 1, i, None);
    // The single-line span ends where the tracker's own step leaves it.
    assert(spec_state_seeded(source, i, None) == spec_fence_next(before, line)) by {
        reveal_with_fuel(spec_state_seeded, 2);
    };
    assert(ends_at(
        source,
        emitted,
        i - 1,
        i,
        spec_state_seeded(source, i - 1, None),
        spec_state_seeded(source, i, None),
    ));
    // The two spans are one step's chunk, so the chain gains one cut.
    lemma_ends_at_join(source, emitted, start, i - 1, i, spec_state_seeded(source, start, None));
    lemma_ends_at_components(
        source,
        emitted,
        start,
        i,
        spec_state_seeded(source, start, None),
        spec_state_seeded(source, i, None),
    );
    lemma_prefix_splits_at_seeded_state(emitted, start, i, None);
    assert(spec_state_seeded(emitted, i, None) == spec_state_seeded(source, i, None));
    assert(next.out.len() == i);
    assert(next.out.len() > prev.out.len());
    lemma_written_extends(source, emitted, rewritable, plan_agenda, i);
    // The tracker left the fence and the line had no marker, so nothing opens.
    lemma_skip_outside_is_unmarked(before, line, rewritable[i - 1], Some(block));
    lemma_skip_leaves_no_fence(before, line);
    assert(spec_state_seeded(source, i, None).is_none());
    reveal(spec_fold_buffered);
    reveal(spec_step_pending);
    assert(next.state.pending.is_none());
    assert(spec_fold_buffered(source, rewritable, i));
    reveal(spec_fold_invariant);
}

} // verus!