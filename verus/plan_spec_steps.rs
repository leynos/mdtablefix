//! One planner step: what it buffers, releases, and writes.
//!
//! The planner decides one line at a time; these are the functions that carry
//! that decision into the fold. [`spec_line_facts`] is the join between the
//! tracker's state machine and the planner's input, and each `spec_step_*`
//! reads one effect of the chosen route: the state it moves to, the block it
//! releases, the line it writes, and how many lines it still owes.
//!
//! Extracted from `verus/plan_spec.rs` at a declaration seam so both files come
//! in under the 400-line ceiling `AGENTS.md` sets. Every item here is `pub`, so
//! the parent re-exports the whole module and no name moves.

use crate::fence_spec::{spec_fence_next};
use crate::production_fence::{FenceState, LineFeatures};
use crate::production_plan::{Agendum, Block as PlanBlock, LineFacts, Release, Step};
use super::{spec_block_agenda, spec_delimiters_eligible, spec_fenced};
use vstd::prelude::*;

verus! {

/// The planner's view of a line's features.
///
/// The caller hands the planner the parsed features only when the line is
/// fence-shaped, and the planner derives its own fencing test from that one
/// value. The reduction is sound because the two producers cannot overlap: a
/// parsed fence run always yields a marker, and the prose fallback never does.
/// The marker's presence is therefore exactly the recognition answer the caller
/// would have given, and the driver can recover that answer from the line alone.
pub open spec fn spec_planner_features(line: LineFeatures) -> Option<LineFeatures> {
    if line.marker.is_some() {
        Some(line)
    } else {
        None
    }
}

/// Everything the caller learned about one line, as the specification computes it.
///
/// This is the join between the tracker's state machine and the planner's input.
/// The two routing answers are read off the fence state before and after the
/// line is stepped; the features are the recognition answer above; and the
/// rewrite flag is the one fact the caller must supply, because it depends on
/// the delimiter grammar rather than on the fence transition.
///
/// Reading the routing answers here rather than trusting a caller's copy is what
/// makes the driver provable: the two answers are functions of the same state
/// recurrence the driver already maintains, so a caller cannot supply answers
/// that disagree with the document it is planning.
pub open spec fn spec_line_facts(
    state: Option<FenceState>,
    line: LineFeatures,
    rewritable: bool,
) -> LineFacts {
    LineFacts {
        was_in_fence: spec_fenced(state, line.depth),
        is_in_fence: spec_fenced(spec_fence_next(state, line), line.depth),
        features: spec_planner_features(line),
        rewritable,
    }
}

/// The agenda a release contributes, given the span of lines it covers.
///
/// The span — not a start and an end — is the parameter because it is what the
/// emitter actually has. A release writes the lines the pass buffered for the
/// block, and the number of those lines is a length the caller already holds; a
/// start and an end would only be two values from which that length is recovered.
/// Naming the length is also what keeps the kernel free of subtraction, which is
/// the one place an underflow could hide.
///
/// The two rewrite flags are the ones the emitter will consult: the opener's was
/// recorded when the block was opened, and the closer's is the releasing line's
/// own. An unterminated block ignores the second flag, which is why the pass can
/// hand that path a value it never reads.
///
/// The block is a parameter rather than being read from the driver state because
/// of exactly one release: a close buffers its line before the block is released,
/// so that line's conflict answer is part of the block being emitted. Every other
/// release leaves the flag alone, because [`spec_absorb`] preserves it.
pub open spec fn spec_released_agenda(
    block: PlanBlock,
    span: int,
    opener_rewritable: bool,
    closer_rewritable: bool,
    release: Release,
) -> Seq<Agendum> {
    spec_block_agenda(
        span,
        release,
        spec_delimiters_eligible(release, opener_rewritable, closer_rewritable),
        !block.conflicts,
    )
}

/// The buffered block after a step, or `None` when nothing is buffered.
pub open spec fn spec_step_pending(step: Step) -> Option<PlanBlock> {
    match step {
        Step::Restart(fresh) => Some(fresh),
        Step::Skip => None,
        Step::Extend(block) => Some(block),
        Step::Close(_, _) => None,
    }
}

/// The rewrite flag a step records for the block it leaves buffered.
///
/// Only an opening delimiter supplies the flag, because only an opening delimiter
/// is ever consulted as one: the eligibility rule reads the block's first line and
/// the line that releases it, and never an interior line. The flag is taken from
/// the line's own facts — the same value the `Restart` that buffered it carried —
/// so a caller cannot record one answer while the planner acted on another.
pub open spec fn spec_step_opener_rewritable(
    opener_rewritable: bool,
    line_rewritable: bool,
    step: Step,
) -> bool {
    match step {
        Step::Restart(_) => line_rewritable,
        Step::Skip => opener_rewritable,
        Step::Extend(_) => opener_rewritable,
        Step::Close(_, _) => opener_rewritable,
    }
}

/// The index a step records for the block it leaves buffered.
///
/// A fresh block begins at the line that opened it, so the step needs the index of
/// the line being planned; every other step carries the previous value through,
/// whether or not a block is buffered, so that the field is total and so that the
/// balance below is an equation about the fold rather than about a case split.
pub open spec fn spec_step_start(start: int, prev_len: int, step: Step) -> int {
    match step {
        Step::Restart(_) => prev_len,
        Step::Skip => start,
        Step::Extend(_) => start,
        Step::Close(_, _) => start,
    }
}

/// The agenda a step contributes for the block it releases, before the line being
/// planned is itself accounted for.
///
/// A step that releases the block the pass was holding writes that block's lines
/// here. The span is `prev_len - start` for the two implicit releases — a restart
/// and a skip both flush the block and then deal with the current line separately,
/// so that line is never part of the block they release — and one more than that
/// for a close, whose line is the block's own closing delimiter.
pub open spec fn spec_step_release(
    pending: Option<PlanBlock>,
    start: int,
    opener_rewritable: bool,
    facts: LineFacts,
    prev_len: int,
    step: Step,
) -> Seq<Agendum> {
    match pending {
        Some(block) => match step {
            Step::Restart(_) => spec_released_agenda(
                block,
                prev_len - start,
                opener_rewritable,
                false,
                Release::Abandoned,
            ),
            Step::Skip => spec_released_agenda(
                block,
                prev_len - start,
                opener_rewritable,
                false,
                Release::Abandoned,
            ),
            Step::Extend(_) => Seq::empty(),
            Step::Close(extended, release) => spec_released_agenda(
                extended,
                prev_len + 1 - start,
                opener_rewritable,
                facts.rewritable,
                release,
            ),
        },
        None => Seq::empty(),
    }
}

/// Whether a step writes the line it is planning, beyond any release.
///
/// A line a close or a restart deals with is already accounted for — by the
/// release that ends the old block, and by the start of the new one — so only a
/// skip writes an agendum here. Between them the release and this flag write every
/// line of the prefix exactly once, which is the whole content of the length
/// theorem below.
pub open spec fn spec_step_writes_line(step: Step) -> bool {
    match step {
        Step::Restart(_) => false,
        Step::Skip => true,
        Step::Extend(_) => false,
        Step::Close(_, _) => false,
    }
}

/// The lines a pending block accounts for when the prefix has reached `i`.
///
/// This is the quantity the length theorem balances: every line of the prefix is
/// either already in the agenda or still inside the block the pass is holding, and
/// the two must add up to the prefix however the fold is stepped. The `end` is not
/// the block's last line but the prefix's, which is why a block one line old
/// accounts for exactly one line only once that line has been stepped.
pub open spec fn spec_pending_lines(pending: Option<PlanBlock>, start: int, end: int) -> int {
    match pending {
        Some(_) => end - start,
        None => 0,
    }
}

} // verus!
