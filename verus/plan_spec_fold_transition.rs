//! What one fold step proves about the lines it buffers or releases.
//!
//! Each lemma here is a case split on [`spec_fence_next`] — the kernel's own
//! transition — read back into the facts the planner's routes need. A route knows
//! only the two routing answers the tracker supplied; these lemmas are what turn
//! those two booleans into the depth, closer, and opening facts the released-span
//! theorem asks for.
//!
//! Nothing here restates the specification: every conclusion is the transition's
//! own definition unfolded at one line. The last lemma assembles the transition
//! facts with the planner's conflict fold, which is the whole of what a step
//! leaving a block buffered has to hand the next step.
//!
//! The planner's fold over a block's interior — the buffered sequence and the
//! conflict accumulations — is in `plan_spec_fold_span.rs`.

use crate::fence_spec::{spec_closes, spec_fence_next, spec_state_seeded};
use crate::production_fence::{FenceState, LineFeatures};
use crate::production_plan::{Block as PlanBlock, Step};

use super::{
    spec_absorb_all, spec_buffered, spec_drive, spec_fenced, spec_fold_buffered, spec_fresh,
    spec_line_facts, spec_masked, spec_open_block, spec_planner_features, spec_plan_step,
};

use vstd::prelude::*;

verus! {

/// A line that is not inside a fence is not masked by that fence's depth.
///
/// [`spec_fenced`] is the tracker's own test, and [`spec_masked`] is the depth
/// drop the transition applies first. The two are complementary: where the tracker
/// says the line is outside, the drop cannot fire, so the line is read against the
/// state as it stands. The fold needs this to hand the released-span theorem a
/// seed at the opener that the masked read-back lemma can accept.
pub proof fn lemma_not_fenced_means_unmasked(
    state: Option<FenceState>,
    depth: usize,
)
    requires !spec_fenced(state, depth),
    ensures spec_masked(state, depth).is_none(),
{
    reveal(spec_fenced);
    reveal(spec_masked);
    match state {
        Some(open) => {
            if depth < open.open_depth {
            } else {
            }
        },
        None => {},
    }
}

/// An opening delimiter's features are exactly the state it opens.
///
/// [`spec_open_block`] succeeds only on a fence-shaped line, and the state it
/// reports is built from that line's marker, run, and depth. Naming the three
/// equalities here means the fold can write them down once, when a block opens,
/// instead of unpacking the option at every later use.
pub proof fn lemma_open_block_establishes(
    state: Option<FenceState>,
    line: LineFeatures,
    rewritable: bool,
    fresh: PlanBlock,
)
    requires spec_open_block(spec_line_facts(state, line, rewritable)) == Some(fresh),
    ensures
        line.marker == Some(fresh.opening.marker),
        line.marker_len == fresh.opening.marker_len,
        line.depth == fresh.opening.open_depth,
        !fresh.conflicts,
{
    reveal(spec_open_block);
    reveal(spec_line_facts);
    reveal(spec_planner_features);
    match line.marker {
        Some(marker) => {},
        None => {},
    }
}

/// A line outside every fence opens the block its marker names.
///
/// This is the transition a restart performs. The tracker was outside the fence,
/// so the depth drop cannot fire, and a fence-shaped line then opens a state of
/// its own marker, run, and depth. The fold records that state as the new block's
/// opening, which is what makes the block's opening facts a consequence of the
/// transition rather than an assumption about it.
pub proof fn lemma_restart_opens(
    state: Option<FenceState>,
    line: LineFeatures,
    marker: char,
)
    requires
        !spec_fenced(state, line.depth),
        line.marker == Some(marker),
    ensures
        spec_fence_next(state, line) == Some(
            FenceState { marker, marker_len: line.marker_len, open_depth: line.depth },
        ),
{
    reveal(spec_fenced);
    reveal(spec_fence_next);
    match state {
        Some(open) => {
            if line.depth < open.open_depth {
            } else {
            }
        },
        None => {},
    }
}

/// A line that leaves its fence open reads as the same state it was read from.
///
/// The `Extend` route is exactly this case: the tracker was inside, and is still
/// inside, so the line neither closed the block nor dropped below it. The three
/// conclusions are the guarded-interior facts for that line — its depth, its not
/// being a closer, and the state it leaves — and they are what the conflict fold
/// is then consulted about.
pub proof fn lemma_extend_keeps(
    open: FenceState,
    line: LineFeatures,
)
    requires
        spec_fenced(Some(open), line.depth),
        spec_fenced(spec_fence_next(Some(open), line), line.depth),
    ensures
        spec_fence_next(Some(open), line) == Some(open),
        line.depth >= open.open_depth,
        !spec_closes(open, line),
{
    reveal(spec_fenced);
    reveal(spec_fence_next);
    reveal(spec_closes);
    if line.depth < open.open_depth {
    } else {
        if spec_closes(open, line) {
        } else {
        }
    }
}

/// A line that takes the tracker out of its fence is that fence's closer.
///
/// The `Close` route's two routing answers are "was inside" and "is not inside",
/// and from a state the line was inside the only way to leave it is the closing
/// branch of the transition. The fold needs the closer relation itself, not the
/// route, because the compressed-block theorem compares the two runs under it.
pub proof fn lemma_close_step(
    open: FenceState,
    line: LineFeatures,
)
    requires
        spec_fenced(Some(open), line.depth),
        !spec_fenced(spec_fence_next(Some(open), line), line.depth),
    ensures
        spec_fence_next(Some(open), line).is_none(),
        spec_closes(open, line),
{
    reveal(spec_fenced);
    reveal(spec_fence_next);
    reveal(spec_closes);
    if line.depth < open.open_depth {
    } else {
        if spec_closes(open, line) {
        } else {
        }
    }
}

/// A line the block absorbed while keeping its fence open is interior content.
///
/// The kernel's transition leaves a state unchanged across a line in exactly two
/// situations: the line is shallower than the fence, or the line closes it. Both
/// leave something other than the same state behind, so getting the state back
/// unchanged is the whole of "the line stayed inside the fence": its depth is at
/// least the opener's, and it is not a closer. These are two of the three
/// guarded-interior facts the released-span theorem asks for; the third is the
/// rewrite permission, which comes from the conflict fold rather than the
/// transition.
pub proof fn lemma_state_stays_means_interior(open: FenceState, line: LineFeatures)
    requires spec_fence_next(Some(open), line) == Some(open),
    ensures
        line.depth >= open.open_depth,
        !spec_closes(open, line),
{
    reveal(spec_fence_next);
    reveal(spec_closes);
    if line.depth < open.open_depth {
    } else {
        if spec_closes(open, line) {
        } else {
        }
    }
}

/// Every line buffered under one opening was interior content of that opening.
///
/// The invariant records, for each line after the opener, the state the line left
/// behind; the two ends of that recurrence are the transition across the line.
/// Reading them together is what turns the recurrence into the depth and closer
/// facts, so a route only has to carry the state equality and not a second copy of
/// the kernel's case analysis.
pub proof fn lemma_interior_shape(
    source: Seq<LineFeatures>,
    opening: FenceState,
    start: int,
    keep: int,
)
    requires
        0 <= start,
        start <= keep <= source.len(),
        forall|j: int|
            #![trigger spec_state_seeded(source, j, None)]
            start < j <= keep ==> spec_state_seeded(source, j, None) == Some(opening),
    ensures
        forall|j: int|
            #![trigger source[j]]
            start < j < keep ==> source[j].depth >= opening.open_depth && !spec_closes(
                opening,
                source[j],
            ),
{
    assert forall|j: int|
        #![trigger source[j]]
        start < j < keep implies source[j].depth >= opening.open_depth && !spec_closes(
            opening,
            source[j],
        ) by {
        // The recurrence at `j + 1` reads the transition across line `j`, and its
        // left-hand end is the state the recurrence at `j` recorded. Naming the
        // two hypotheses first is what puts `source[j + 1]` — the recurrence's
        // trigger — into the query.
        assert(source[j + 1] == source[j + 1]);
        assert(spec_state_seeded(source, j + 1, None) == Some(opening));
        assert(spec_state_seeded(source, j, None) == Some(opening));
        assert(spec_state_seeded(source, j + 1, None) == spec_fence_next(
            spec_state_seeded(source, j, None),
            source[j],
        )) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        assert(spec_fence_next(Some(opening), source[j]) == Some(opening));
        lemma_state_stays_means_interior(opening, source[j]);
    }
}

/// A line outside every fence that is not fence-shaped leaves no fence open.
///
/// The `Skip` route with nothing buffered. The tracker was outside, so the depth
/// drop cannot fire, and a line with no marker opens nothing. The fold uses this
/// to carry "no block is buffered" across a directly written line.
pub proof fn lemma_skip_leaves_no_fence(
    state: Option<FenceState>,
    line: LineFeatures,
)
    requires
        !spec_fenced(state, line.depth),
        line.marker.is_none(),
    ensures spec_fence_next(state, line).is_none(),
{
    reveal(spec_fenced);
    reveal(spec_fence_next);
    match state {
        Some(open) => {
            if line.depth < open.open_depth {
            } else {
            }
        },
        None => {},
    }
}

/// A line the planner skipped from outside a fence had no marker to open.
///
/// A skip is the route that writes a line through on its own. From outside every
/// fence there are two ways to reach it — with nothing buffered, and by leaving a
/// buffered block — and both end at the same conclusion: the line opened nothing,
/// so the tracker is left holding nothing. The fold needs that conclusion rather
/// than the route, because "nothing is buffered" is what the next step's guard
/// reads.
pub proof fn lemma_skip_outside_is_unmarked(
    state: Option<FenceState>,
    line: LineFeatures,
    rewritable: bool,
    block: Option<PlanBlock>,
)
    requires
        !spec_fenced(state, line.depth),
        spec_plan_step(block, spec_line_facts(state, line, rewritable)) == Step::Skip,
    ensures line.marker.is_none(),
{
    reveal(spec_plan_step);
    reveal(spec_line_facts);
    reveal(spec_planner_features);
    reveal(spec_open_block);
    reveal(spec_fenced);
    match line.marker {
        Some(_) => {
            assert(spec_open_block(spec_line_facts(state, line, rewritable)).is_some());
            assert(false);
        },
        None => {},
    }
}

/// The facts a fold step inherits from the block it is already holding.
///
/// The buffer half of the invariant at the shorter prefix is five statements about
/// a block: where it began, what its opening line said, that the seed it was read
/// from is not masked, that every line since has left the same fence open, and what
/// the planner's conflict fold over those lines answered. Every route needs all
/// five, and none depends on which route is about to be taken.
///
/// The state at the current line is added here rather than derived per route: it is
/// the recurrence's last known point, and every route either extends it or ends it.
pub proof fn lemma_buffered_context(
    source: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    i: int,
    block: PlanBlock,
)
    requires
        0 < i <= source.len(),
        i <= rewritable.len(),
        spec_fold_buffered(source, rewritable, i - 1),
        spec_drive(source, rewritable, i - 1).state.pending == Some(block),
    ensures
        spec_drive(source, rewritable, i - 1).state.start < i - 1,
        spec_state_seeded(source, i - 1, None) == Some(block.opening),
        source[spec_drive(source, rewritable, i - 1).state.start].marker == Some(
            block.opening.marker,
        ),
        source[spec_drive(source, rewritable, i - 1).state.start].marker_len
            == block.opening.marker_len,
        source[spec_drive(source, rewritable, i - 1).state.start].depth
            == block.opening.open_depth,
        spec_masked(
            spec_state_seeded(source, spec_drive(source, rewritable, i - 1).state.start, None),
            block.opening.open_depth,
        ).is_none(),
        forall|j: int|
            #![trigger spec_state_seeded(source, j, None)]
            spec_drive(source, rewritable, i - 1).state.start < j <= i - 1
                ==> spec_state_seeded(source, j, None) == Some(block.opening),
        block.conflicts == spec_absorb_all(
            spec_fresh(block.opening),
            spec_buffered(source, spec_drive(source, rewritable, i - 1).state.start + 1, i - 1),
            i - 1 - spec_drive(source, rewritable, i - 1).state.start - 1,
        ).conflicts,
{
    reveal(spec_fold_buffered);
    // The recurrence's last known point is the one the step is about to consume,
    // so it is the instance of the invariant's quantifier at `i - 1`. Naming the
    // line's features puts the quantifier's trigger in the goal.
    assert(source[i - 1] == source[i - 1]);
    assert(spec_state_seeded(source, i - 1, None) == Some(block.opening));
}

} // verus!