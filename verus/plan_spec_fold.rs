//! The fold that drives the planner over a document.
//!
//! [`spec_drive`] is the recursion, [`spec_step_state`] and
//! [`spec_step_agenda`] are its per-step effects, and [`spec_finish`] reads the
//! tail a completed fold leaves behind. [`spec_document_plan`] is the pass's
//! whole decision for a document, and [`lemma_document_plan_total`] is its
//! totality: the plan is defined for every source.
//!
//! Extracted from `verus/plan_spec.rs` at a declaration seam so both files come
//! in under the 400-line ceiling `AGENTS.md` sets. [`lemma_finish_total`] is
//! private and stays so: its only caller is `lemma_document_plan_total` in this
//! same module.

use crate::fence_spec::{spec_state_seeded};
use crate::production_fence::{LineFeatures};
use crate::production_plan::{Agendum, Block as PlanBlock, LineFacts, Release};
use super::{lemma_drive_length, lemma_released_agenda_length, spec_balanced};
use super::{spec_line_facts, spec_pending_lines, spec_released_agenda, spec_step_opener_rewritable, spec_step_pending, spec_step_release, spec_step_start, spec_step_writes_line};
use super::{spec_plan_step};
use vstd::prelude::*;

verus! {

/// The buffering half of the driver's state: the three fields a step changes.
///
/// This is the part the production kernel carries, and splitting it out is what
/// makes the kernel's postcondition statable: the kernel neither receives nor
/// returns the accumulated agenda, because the emitter appends to the document
/// itself. The specification's step and agenda functions are stated over this
/// record, and the fold below adds the agenda, so the two halves are joined in the
/// specification rather than in the kernel's argument list.
pub struct DriveState {
    /// The block being buffered, or `None` between blocks.
    pub pending: Option<PlanBlock>,
    /// Index of the pending block's opening line.
    pub start: int,
    /// Whether the pending block's opening line was a rewrite candidate.
    pub opener_rewritable: bool,
}

/// The driver's state after some prefix of a document.
///
/// The pass's loop is a fold over four values, and the whole document theorem
/// needs all four to say what the emitted sequence is: the three buffering fields
/// above, and the agenda accumulated so far. The fence state is deliberately *not*
/// a field: it is [`spec_state_seeded`] of the same prefix, which is what lets the
/// driver reuse the transition kernel's own specification rather than restating
/// it.
pub struct Drive {
    /// The buffering state, which the production kernel computes.
    pub state: DriveState,
    /// The agenda for every line the fold has accounted for so far.
    pub out: Seq<Agendum>,
}

/// The buffering state a step leaves behind.
///
/// The step is the planner's own routing relation applied to the line's facts, so
/// the branch taken here is the branch the production driver takes and not a model
/// of it. The three fields are the whole of what a step changes: which block is
/// buffered, where that block began, and whether its opener was a rewrite
/// candidate.
///
/// Taking the line's [`LineFacts`] rather than the fence state the line was read
/// against is deliberate. A production caller has already parsed the line and holds
/// exactly these facts; asking for them keeps the kernel's argument list equal to
/// the value the emitter already possesses, and moves the question of whether those
/// facts are the ones the document implies to the loop that produces them.
pub open spec fn spec_step_state(prev: DriveState, facts: LineFacts, prev_len: int) -> DriveState {
    let step = spec_plan_step(prev.pending, facts);
    DriveState {
        pending: spec_step_pending(step),
        start: spec_step_start(prev.start, prev_len, step),
        opener_rewritable: spec_step_opener_rewritable(
            prev.opener_rewritable,
            facts.rewritable,
            step,
        ),
    }
}

/// The agenda a step contributes.
///
/// This is the whole of what the pass writes for the lines a step accounts for, and
/// it is what the production kernel returns for the emitter to append: the release
/// of the block the step ends, plus the line itself when the line is written
/// directly rather than buffered. The length theorem below is the statement that
/// these contributions sum to the document.
pub open spec fn spec_step_agenda(prev: DriveState, facts: LineFacts, prev_len: int) -> Seq<Agendum> {
    let step = spec_plan_step(prev.pending, facts);
    let released = spec_step_release(
        prev.pending,
        prev.start,
        prev.opener_rewritable,
        facts,
        prev_len,
        step,
    );
    if spec_step_writes_line(step) {
        released.push(Agendum::Verbatim)
    } else {
        released
    }
}

/// The driver fold advanced by one line.
pub open spec fn spec_drive_step(prev: Drive, facts: LineFacts, prev_len: int) -> Drive {
    Drive {
        state: spec_step_state(prev.state, facts, prev_len),
        out: prev.out.add(spec_step_agenda(prev.state, facts, prev_len)),
    }
}

/// The driver fold, stated as a recursion over the prefix length.
///
/// This is the pass's whole decision procedure with the text stripped out. The
/// line the fold is extending by is read against [`spec_state_seeded`] of the
/// shorter prefix, which is the tracker state after that many lines, and the step
/// it takes is [`spec_drive_step`] — so the fold and the production loop differ
/// only in whether the tracker supplies that state or a recursion recomputes it.
///
/// The four cases of the planner step are exactly the pass's four dispositions of
/// a line: it opens a block, it is emitted directly, it is buffered into the open
/// block, or it closes the block and is released with it. Every flush path the
/// pass has is one of them. Two of the four release a pending block: a skip and a
/// restart both end it implicitly, which is the pass's abandoned release, and a
/// close ends it at a delimiter. The document's own end inside a block is the
/// fourth flush path, and [`spec_finish`] adds it once the fold is complete.
pub open spec fn spec_drive(
    source: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    i: int,
) -> Drive
    decreases i
{
    if i <= 0 {
        Drive {
            state: DriveState { pending: None, start: 0, opener_rewritable: false },
            out: Seq::empty(),
        }
    } else {
        spec_drive_step(
            spec_drive(source, rewritable, i - 1),
            spec_line_facts(
                spec_state_seeded(source, i - 1, None),
                source[i - 1],
                rewritable[i - 1],
            ),
            i - 1,
        )
    }
}

/// The pass's whole decision for a document.
///
/// The agenda is one agendum per input line, which is the line-count half of the
/// document theorem: the driver records a single agendum for every line it writes
/// directly and a block's whole agenda for every line it buffers, so the emitted
/// document is exactly as long as the source.
///
/// The other two fields are the block the document ended inside, if it did, and
/// that block's line count. They are not decoration: the pass reports exactly
/// these two values when it preserves an unterminated block, so stating them here
/// is what makes the diagnostic a consequence of the decision rather than a
/// separate observation the adapter makes for itself.
pub struct Plan {
    /// What the pass writes, one agendum per source line.
    pub agenda: Seq<Agendum>,
    /// The block left open at end of input, or `None`.
    pub unclosed: Option<PlanBlock>,
    /// That block's line count, zero when there is none.
    pub unclosed_lines: int,
}

/// The block a completed fold leaves unclosed, read out as the plan's tail.
///
/// A fold that stops inside a block is not a failure: the document ended with a
/// delimiter still open, and the pass writes that block's lines exactly as a close
/// would have, with the one difference that there is no closing delimiter to
/// rewrite. This is the fourth flush path, and it is the only place the release
/// `Unmatched` is produced — which is why the pass can write `false` for a closer
/// flag it never consults.
pub open spec fn spec_finish(drive: Drive, len: int) -> Plan {
    match drive.state.pending {
        Some(block) => Plan {
            agenda: drive.out.add(spec_released_agenda(
                block,
                len - drive.state.start,
                drive.state.opener_rewritable,
                false,
                Release::Unmatched,
            )),
            unclosed: Some(block),
            unclosed_lines: len - drive.state.start,
        },
        None => Plan { agenda: drive.out, unclosed: None, unclosed_lines: 0 },
    }
}

/// The pass's whole decision for a document.
///
/// Stated as the fold followed by the tail, which is how the pass is written. The
/// two halves are proved separately — [`lemma_drive_length`] for the fold and
/// [`lemma_finish_total`] for the tail — because they are separate obligations: the
/// fold is where every buffering and release decision is made, and the tail is the
/// one flush path that has no releasing line to decide anything about.
pub open spec fn spec_document_plan(
    source: Seq<LineFeatures>,
    rewritable: Seq<bool>,
) -> Plan {
    spec_finish(spec_drive(source, rewritable, source.len() as int), source.len() as int)
}

/// `LEM-FINISH-TOTAL` — the tail adds the remaining lines and nothing else.
///
/// The fold's balance says the prefix is covered by the agenda plus the pending
/// block. The tail releases exactly that block, over the span the balance measured,
/// so the agenda after the tail holds one agendum per source line — the total length
/// equation the emitter relies on. A fold that stopped between blocks adds nothing
/// at all, which is the `None` arm and the reason the equation holds for documents
/// that end outside a fence as well as inside one.
pub(super) proof fn lemma_finish_total(drive: Drive, len: int)
    requires spec_balanced(drive, len),
    ensures
        spec_finish(drive, len).agenda.len() == len,
        spec_finish(drive, len).unclosed_lines <= len,
{
    reveal(spec_finish);
    reveal(spec_balanced);
    reveal(spec_pending_lines);
    match drive.state.pending {
        Some(block) => {
            lemma_released_agenda_length(
                block,
                len - drive.state.start,
                drive.state.opener_rewritable,
                false,
                Release::Unmatched,
            );
        },
        None => {},
    }
}

/// `LEM-DOCUMENT-PLAN-TOTAL` — the decision is defined and complete on every input.
///
/// The fold reads each line at the index the plan's own sequence gives it and takes
/// every routing answer from the sequence's prefix, so nothing in the decision can
/// fail to find what it needs. The conclusion is the length equation the emitter
/// relies on, including for the empty document, whose fold is the base case and
/// whose plan has no unclosed block.
///
/// Public because the empty-document witness in
/// `verus/plan_spec_witnesses.rs` discharges its hypothesis by instantiating
/// this result at the empty source rather than re-deriving the boundary case.
pub proof fn lemma_document_plan_total(source: Seq<LineFeatures>, rewritable: Seq<bool>)
    requires source.len() == rewritable.len(),
    ensures
        spec_document_plan(source, rewritable).agenda.len() == source.len(),
        spec_document_plan(source, rewritable).unclosed_lines <= source.len(),
{
    let i = source.len() as int;
    lemma_drive_length(source, rewritable, i);
    reveal(spec_document_plan);
    lemma_finish_total(spec_drive(source, rewritable, i), i);
}

} // verus!
