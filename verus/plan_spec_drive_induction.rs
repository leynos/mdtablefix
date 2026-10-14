//! The driver length theorem and the invariant it establishes.
//!
//! The planner's fold buffers, releases and writes as it consumes a document.
//! This file states that accounting precisely — [`spec_balanced`] — and proves
//! that every prefix of a drive satisfies it, by induction on the prefix length
//! ([`lemma_drive_length`]).
//!
//! The theorem is stated here rather than beside the fold itself because the
//! induction is a self-contained argument: the three length lemmas below reduce
//! each kind of step to an equation about the agenda constructor, and then the
//! induction itself is arithmetic over those equations. `verus/plan_spec.rs`
//! states what the fold is; this file establishes how much of the input it has
//! accounted for at every point.
//!
//! Extracted from `verus/plan_spec.rs` so both files come in under the 400-line
//! ceiling `AGENTS.md` sets. The parent reaches every item here through a
//! private glob, so the document theorem that consumes them is unchanged and
//! the items keep the visibility they had before the move.

use vstd::prelude::*;

use super::{
    lemma_block_agenda_length, spec_delimiters_eligible, spec_drive, spec_drive_step,
    spec_line_facts, spec_pending_lines, spec_plan_step, spec_released_agenda, spec_step_agenda,
    spec_step_pending, spec_step_release, spec_step_start, spec_step_state, spec_step_writes_line,
    Drive, DriveState,
};
use crate::fence_spec::spec_state_seeded;
use crate::production_fence::LineFeatures;
use crate::production_plan::{Block as PlanBlock, LineFacts, Release, Step};

verus! {


/// `INV-DRIVER-BALANCE` — the fold's agenda and its pending block cover the prefix.
///
/// The invariant the length theorem establishes, in its three parts. Every line of
/// the prefix is accounted for exactly once — by an agendum the fold has recorded or
/// by the span of the block it is still holding — so the two add up to the prefix.
/// The block's opening index is never beyond the prefix. And while a block is
/// buffered no outside line has been released after it, which is why the agenda's
/// length is exactly the opening index; that third part is what makes the induction
/// below arithmetic rather than an accumulating argument.
pub open spec fn spec_balanced(drive: Drive, i: int) -> bool {
    drive.state.start <= i
        && drive.out.len() + spec_pending_lines(drive.state.pending, drive.state.start, i) == i
        && (drive.state.pending.is_some() ==> drive.out.len() == drive.state.start)
}

/// `LEM-RELEASE-ACCOUNTS-FOR-THE-BLOCK`.
///
/// A release contributes exactly as many agenda as the span it covers. This is
/// [`lemma_block_agenda_length`] read through the driver: it is the step that makes
/// the whole-prefix balance an equation about the fold rather than about the agenda
/// constructor, and it is stated once so that neither the step lemma below nor the
/// induction has to re-derive the block's length from the emission rule.
pub(super) proof fn lemma_released_agenda_length(
    block: PlanBlock,
    span: int,
    opener_rewritable: bool,
    closer_rewritable: bool,
    release: Release,
)
    requires span >= 0,
    ensures spec_released_agenda(
        block,
        span,
        opener_rewritable,
        closer_rewritable,
        release,
    ).len() == span,
{
    reveal(spec_released_agenda);
    reveal(spec_delimiters_eligible);
    lemma_block_agenda_length(
        span,
        release,
        spec_delimiters_eligible(release, opener_rewritable, closer_rewritable),
        !block.conflicts,
        span,
    );
}

/// The release a step contributes holds as many agenda as the block it flushes.
///
/// The three releases that write a block each contribute its own span, and the step
/// that merely buffers contributes nothing. Naming the count per case here is what
/// lets the balance below be arithmetic rather than a case analysis repeated at every
/// induction step — the case split is discharged once, here, and the induction only
/// has to know which branch of the route it is in.
pub(super) proof fn lemma_step_release_length(
    pending: Option<PlanBlock>,
    start: int,
    opener_rewritable: bool,
    facts: LineFacts,
    prev_len: int,
    step: Step,
)
    requires prev_len >= start,
    ensures
        spec_step_release(
            pending,
            start,
            opener_rewritable,
            facts,
            prev_len,
            step,
        ).len() == match pending {
            Some(_) => match step {
                Step::Restart(_) => prev_len - start,
                Step::Skip => prev_len - start,
                Step::Extend(_) => 0,
                Step::Close(_, _) => prev_len + 1 - start,
            },
            None => 0,
        },
{
    reveal(spec_step_release);
    match pending {
        Some(block) => match step {
            Step::Restart(_) => {
                lemma_released_agenda_length(
                    block,
                    prev_len - start,
                    opener_rewritable,
                    false,
                    Release::Abandoned,
                );
            },
            Step::Skip => {
                lemma_released_agenda_length(
                    block,
                    prev_len - start,
                    opener_rewritable,
                    false,
                    Release::Abandoned,
                );
            },
            Step::Extend(_) => {},
            Step::Close(extended, release) => {
                lemma_released_agenda_length(
                    extended,
                    prev_len + 1 - start,
                    opener_rewritable,
                    facts.rewritable,
                    release,
                );
            },
        },
        None => {},
    }
}

/// How many agenda one step contributes.
///
/// This is the length of [`spec_step_agenda`] spelled out per branch of the route,
/// so the induction below can add up the two halves — the block a step releases and
/// the line it writes — without unfolding either constructor again.
pub(super) proof fn lemma_step_agenda_length(
    state: DriveState,
    facts: LineFacts,
    prev_len: int,
    step: Step,
)
    requires step == spec_plan_step(state.pending, facts), prev_len >= state.start,
    ensures
        spec_step_agenda(state, facts, prev_len).len() == match state.pending {
            Some(_) => match step {
                Step::Restart(_) => prev_len - state.start,
                Step::Skip => prev_len - state.start + 1,
                Step::Extend(_) => 0,
                Step::Close(_, _) => prev_len + 1 - state.start,
            },
            None => match step {
                Step::Skip => 1,
                _ => 0,
            },
        },
{
    reveal(spec_step_agenda);
    reveal(spec_step_writes_line);
    lemma_step_release_length(
        state.pending,
        state.start,
        state.opener_rewritable,
        facts,
        prev_len,
        step,
    );
}

/// The route a planner takes with nothing buffered.
///
/// A step with `None` buffered can only be an opening or a direct write: buffering
/// into a block presupposes a block, and closing one presupposes a delimiter that
/// ended it. Naming the fact here lets the induction below discharge those two
/// branches without unfolding the planner again.
pub(super) proof fn lemma_no_block_cannot_buffer_or_close(pending: Option<PlanBlock>, facts: LineFacts)
    requires pending.is_none(),
    ensures !matches!(spec_plan_step(pending, facts), Step::Extend(_) | Step::Close(_, _)),
{
    reveal(spec_plan_step);
}

/// `LEM-DRIVE-LENGTH` — the fold's agenda and its pending block cover the prefix.
///
/// The line-count half of the document theorem, established on the decision itself
/// rather than on the loop that computes it. Each of the four dispositions accounts
/// for exactly the lines it consumes — a buffered line joins the pending span, an
/// opening line joins the new block's span, a line emitted directly becomes one
/// agendum, and a close turns the whole block into agenda of its own length — so the
/// agenda's length and the pending block's span always sum to the prefix, whatever
/// route the planner took.
///
/// The empty prefix is the base case and satisfies the balance outright: no block is
/// buffered and no line is claimed. That is not a degenerate case to be waved away —
/// it is the document consisting of no lines, and it is one of the inputs the
/// witnesses below name.
pub(super) proof fn lemma_drive_length(source: Seq<LineFeatures>, rewritable: Seq<bool>, i: int)
    requires
        0 <= i,
        i <= source.len(),
        i <= rewritable.len(),
    ensures spec_balanced(spec_drive(source, rewritable, i), i),
    decreases i,
{
    if i > 0 {
        lemma_drive_length(source, rewritable, i - 1);
        let prev = spec_drive(source, rewritable, i - 1);
        let line = source[i - 1];
        let facts = spec_line_facts(
            spec_state_seeded(source, i - 1, None),
            line,
            rewritable[i - 1],
        );
        let step = spec_plan_step(prev.state.pending, facts);
        let next = spec_drive_step(prev, facts, i - 1);
        let added = spec_step_agenda(prev.state, facts, i - 1).len();
        reveal(spec_drive);
        reveal(spec_drive_step);
        reveal(spec_step_state);
        reveal(spec_balanced);
        reveal(spec_step_pending);
        reveal(spec_step_start);
        assert(spec_drive(source, rewritable, i) == next) by {
            reveal_with_fuel(spec_drive, 2);
        };
        assert(prev.state.start <= i - 1);
        assert(prev.out.len() + added == next.out.len()) by {
            reveal(spec_step_agenda);
        };
        lemma_step_agenda_length(prev.state, facts, i - 1, step);

        // Everything below is which route was taken: the two halves of the balance
        // are named by the step-agenda lemma, so each branch only has to say where
        // the new block's span starts and how long the new agenda is.

        match prev.state.pending {
            Some(block) => {
                assert(prev.out.len() == prev.state.start);
                match step {
                    Step::Restart(_) => {
                        assert(added == i - 1 - prev.state.start) by (nonlinear_arith)
                            requires added == i - 1 - prev.state.start;
                        assert(next.state.start == i - 1);
                        assert(next.out.len() == i - 1) by (nonlinear_arith)
                            requires next.out.len() == prev.state.start + (i - 1 - prev.state.start),
                                prev.state.start <= i - 1;
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == 1) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                    Step::Skip => {
                        assert(added == i - 1 - prev.state.start + 1);
                        assert(next.state.start == prev.state.start);
                        assert(next.state.pending.is_none());
                        assert(next.out.len() == i) by (nonlinear_arith)
                            requires next.out.len() == prev.state.start + (i - 1 - prev.state.start + 1),
                                prev.state.start <= i - 1;
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == 0) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                    Step::Extend(_) => {
                        assert(added == 0);
                        assert(next.state.start == prev.state.start);
                        assert(next.out.len() == prev.state.start);
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == i
                            - prev.state.start) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                    Step::Close(_, _) => {
                        assert(added == i - 1 + 1 - prev.state.start);
                        assert(next.state.start == prev.state.start);
                        assert(next.state.pending.is_none());
                        assert(next.out.len() == i) by (nonlinear_arith)
                            requires next.out.len() == prev.state.start + (i - 1 + 1 - prev.state.start);
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == 0) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                }
            },
            None => {
                assert(prev.out.len() == i - 1) by {
                    reveal(spec_pending_lines);
                };
                lemma_no_block_cannot_buffer_or_close(prev.state.pending, facts);
                match step {
                    Step::Restart(_) => {
                        assert(added == 0);
                        assert(next.state.start == i - 1) by {
                            reveal(spec_step_start);
                        };
                        assert(next.out.len() == i - 1);
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == 1) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                    Step::Skip => {
                        assert(added == 1) by {
                            reveal(spec_step_writes_line);
                        };
                        assert(next.state.start == prev.state.start);
                        assert(next.state.pending.is_none());
                        assert(next.out.len() == i) by {
                            reveal(spec_step_agenda);
                            reveal(spec_step_writes_line);
                        };
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == 0) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                    Step::Extend(_) => {
                        assert(false);
                    },
                    Step::Close(_, _) => {
                        assert(false);
                    },
                }
            },
        }
    }
}

} // verus!
