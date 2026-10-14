//! The planner's driver loop: buffering state, and the agenda each step releases.
//!
//! `plan.rs` decides what a single line means — whether it is a delimiter,
//! whether it may be respelled, and whether it closes the open block. This file
//! carries those decisions across a document. It holds the state the production
//! loop keeps between lines: which block is buffered, where it began, and
//! whether its opener was a rewrite candidate. It also computes the agenda each
//! step contributes for the lines that step accounts for.
//!
//! The division is by question rather than by size, and nothing here re-decides
//! anything `plan.rs` decided. Each function is one step of the fold
//! [`crate::spec_drive`] describes, and [`drive_step`] is the step the
//! production loop calls once per line. A step returns its state and its agenda
//! together so a caller cannot carry one forward and drop the other, which is
//! what keeps a lost line visible to the length theorem rather than silent.
//!
//! Both builds reach this file through the declaration in `plan.rs`, so the
//! library's child module and `verus/lib.rs`'s `production_plan` compile the
//! same source rather than two copies free to drift.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

use super::{
    Agendum,
    Block,
    LineFacts,
    Release,
    Step,
    block_agenda,
    delimiters_eligible,
    plan_step,
};

/// The buffering state of the pass's driver loop, after some prefix of a document.
///
/// This is [`spec_drive`](crate::spec_drive)'s four fields with the two the kernel
/// does not need removed: the emitter holds the lines and derives them from the
/// span it is given, and the accumulated agenda is what the kernel returns rather
/// than what it is handed. What remains is exactly the bookkeeping a caller cannot
/// recompute — which block is open, where it began, and whether its opener was a
/// rewrite candidate — because all three are answers the planner made on lines the
/// caller has already buffered.
#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, PartialEq, Eq))]
pub struct DriverState {
    /// The block being buffered, or `None` between blocks.
    pub pending: Option<Block>,
    /// Index of the pending block's opening line.
    pub start: usize,
    /// Whether the pending block's opening line was a rewrite candidate.
    pub opener_rewritable: bool,
}

impl DriverState {
    /// The state a document begins in: no block buffered, no opening line.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pending: None,
            start: 0,
            opener_rewritable: false,
        }
    }
}

impl Default for DriverState {
    fn default() -> Self { Self::new() }
}

verified_kernel_function! {
/// Advance the driver's buffering state by one line, and name what to emit.
///
/// The line arrives already parsed, exactly as the production loop has it, and the
/// step is the planner's own routing relation — so the kernel and the pass cannot
/// route a line differently. What the kernel returns is the pair the pass acts on:
/// the state to carry into the next call, and the agenda for the lines this step
/// accounts for, which is the release of any block the step ends plus the line
/// itself when the line is written directly.
///
/// The state is returned rather than mutated because the pass holds it as one
/// value, and the agenda is returned rather than appended because the pass appends
/// it to the document. Splitting the two would let a caller update the state and
/// drop the agenda, which is a way of losing a line that the length theorem would
/// then be unable to see.
///
/// The caller must carry this state's successor into the next call, and must
/// append every returned agendum exactly once. Both obligations are discharged by
/// [`crate::lemma_drive_length`], which is stated over [`crate::spec_drive`] — the
/// fold this kernel computes one step of — rather than over a copy of it.
///
/// The agenda is not accompanied by a description of which lines it covers, and the
/// caller does not need one: the step's agenda holds one agendum for every line the
/// step accounts for and no more, so a caller that appends the line to its buffer
/// before writing the agenda finds the buffer's first entries are exactly the ones
/// the agenda names. That is [`lemma_step_agenda_length`] read forward, and it is
/// what lets the adapter write lines without branching on the route.
#[must_use]
pub fn drive_step(
    state: DriverState,
    facts: LineFacts,
    index: usize,
) -> (DriverState, Vec<Agendum>);
requires(state.start <= index, index < usize::MAX);
ensures(result => result.0@ == crate::spec_step_state(state@, facts@, index as int)
&& result.1@ == crate::spec_step_agenda(state@, facts@, index as int));
{
    let held: Option<Block> = state.pending;
    let step = plan_step(held, facts);
    // Every field a step carries through is named for the whole family that
    // carries it, so the arms that agree read as one rule rather than as three
    // that happen to coincide. The specification states these field by field; the
    // or-patterns here are the same function, and `spec_step_pending` and its
    // siblings remain the authority on what the values are.
    let pending = match step {
        Step::Restart(fresh) => Some(fresh),
        Step::Extend(block) => Some(block),
        Step::Skip | Step::Close(_, _) => None,
    };
    let start = match step {
        Step::Restart(_) => index,
        Step::Skip | Step::Extend(_) | Step::Close(_, _) => state.start,
    };
    let opener_rewritable = match step {
        Step::Restart(_) => facts.rewritable,
        Step::Skip | Step::Extend(_) | Step::Close(_, _) => state.opener_rewritable,
    };
    let agenda = release_agenda(held, state.start, state.opener_rewritable, facts, index, step);
    let mut out: Vec<Agendum> = agenda;
    if let Step::Skip = step {
        out.push(Agendum::Verbatim);
    }
    (DriverState { pending, start, opener_rewritable }, out)
}
}

verified_kernel_function! {
/// The agenda a step contributes for the block it releases.
///
/// A step that ends the block the pass was holding writes that block's lines here,
/// and a step that does not writes nothing. The span is the block's own — from the
/// opening line up to the line before the current one for the two implicit
/// releases, and up to and including the current line for a close, whose line is
/// the block's own closing delimiter.
///
/// The block released is not always the block the caller held. A close buffers its
/// line before releasing the block, and a closing delimiter can carry a conflict of
/// its own, so the extended block is what must be emitted; every other step leaves
/// the conflict flag alone, which is why the caller can pass the held block through
/// unchanged.
#[must_use]
pub fn release_agenda(
    held: Option<Block>,
    start: usize,
    opener_rewritable: bool,
    facts: LineFacts,
    index: usize,
    step: Step,
) -> Vec<Agendum>;
requires(start <= index, index < usize::MAX);
ensures(result => result@ == crate::spec_step_release(
    held@,
    start as int,
    opener_rewritable,
    facts@,
    index as int,
    step,
));
{
    let Some(block) = held else {
        return Vec::new();
    };
    match step {
        Step::Restart(_) | Step::Skip => released_agenda(
            block,
            index - start,
            opener_rewritable,
            false,
            Release::Abandoned,
        ),
        Step::Extend(_) => Vec::new(),
        Step::Close(extended, release) => released_agenda(
            extended,
            index - start + 1,
            opener_rewritable,
            facts.rewritable,
            release,
        ),
    }
}
}

verified_kernel_function! {
/// The agenda for a released block, given the span it covers.
///
/// This is the emitter's whole decision, and it is shared by every flush path: the
/// pass writes a block's lines verbatim except at the delimiters the release puts
/// in play, and those take the compressed spelling only when the block has no
/// conflicting interior line. Naming the span rather than the block's length is
/// what lets a close pass a span that includes the line it just buffered.
#[must_use]
pub fn released_agenda(
    block: Block,
    span: usize,
    opener_rewritable: bool,
    closer_rewritable: bool,
    release: Release,
) -> Vec<Agendum>;
ensures(result => result@ == crate::spec_released_agenda(
    block@,
    span as int,
    opener_rewritable,
    closer_rewritable,
    release,
));
{
    block_agenda(
        span,
        release,
        delimiters_eligible(release, opener_rewritable, closer_rewritable),
        !block.conflicts,
    )
}
}
