//! The fold's invariant: what the driver has established after a prefix.
//!
//! [`spec_fold_invariant`] is the statement the document theorem needs to be
//! true at every prefix. It is not decoration: each conjunct is consumed by the
//! released-span theorem at the next release, so weakening one makes a later
//! proof fail rather than leaving the tree clean.
//!
//! The invariant has four kinds of content. The *accounting* part is
//! [`spec_balanced`] plus the correspondence between the fold's agenda so far and
//! the whole plan's agenda — every released agendum at index `j` belongs to
//! source line `j`. The *preservation* part is the agreement of the two runs in
//! state over the prefix the fold has written. The *span* part is the chain of
//! completed spans, accumulated so the composition law can join them at the end.
//! The *buffer* part describes the block the fold is holding: its opener's
//! transition facts, the state every buffered line left behind, and the planner's
//! own conflict fold over the lines it absorbed.
//!
//! Why the buffer part is stated over the fold rather than assumed: the
//! compressed-block theorem needs "no interior line conflicts" and "each interior
//! line is not a closer". Both are facts about the planner's decisions, and the
//! only place they are decided is [`spec_absorb_all`] over the buffered lines.
//! Deriving them here is what keeps the document theorem from assuming the very
//! condition it is supposed to establish.

use crate::fence_spec::spec_state_seeded;
use crate::production_fence::{FenceState, LineFeatures};
use crate::production_plan::Agendum;

use super::{
    spec_absorb_all, spec_balanced, spec_buffered, spec_chain_of, spec_chain_preserved, spec_drive,
    spec_fresh, spec_masked,
};

use vstd::prelude::*;

verus! {

/// Whether `prefix` is the initial segment of `total`.
///
/// The fold's agenda grows by appending, so at any prefix it is an initial
/// segment of the plan's final agenda. Stating that is what lets a step read the
/// agendum it is writing out of the *whole* plan, which is where the emission
/// contract is stated, without the step's own reasoning having to know what the
/// rest of the document will do. The relation is transitivity-friendly: an
/// initial segment of an initial segment is an initial segment.
pub open spec fn spec_extends(prefix: Seq<Agendum>, total: Seq<Agendum>) -> bool {
    prefix.len() <= total.len() && total.subrange(0, prefix.len() as int) == prefix
}

/// A position inside an initial segment has the same value in the whole.
///
/// This is the projection the step uses to read the emission contract: the
/// agendum the driver chose at position `base + k` is the agendum the plan's
/// agenda holds at that position, so the contract stated over the plan applies
/// to the chunk the step is writing.
pub(super) proof fn lemma_extends_index(
    prefix: Seq<Agendum>,
    total: Seq<Agendum>,
    k: int,
)
    requires
        spec_extends(prefix, total),
        0 <= k < prefix.len(),
    ensures total[k] == prefix[k],
{
    reveal(spec_extends);
    vstd::seq::axiom_seq_subrange_index(total, 0, prefix.len() as int, k);
}

/// An appended chunk leaves the shorter agenda an initial segment of the longer.
///
/// The fold produces the longer agenda by `prefix.add(chunk)`, so this is the
/// direction the induction needs when it re-derives the invariant for the shorter
/// prefix from the longer one: whatever the fold has written at a prefix stays an
/// initial segment of everything it writes afterwards.
pub(super) proof fn lemma_extends_add_left(prefix: Seq<Agendum>, chunk: Seq<Agendum>)
    ensures spec_extends(prefix, prefix.add(chunk)),
{
    reveal(spec_extends);
    vstd::seq::axiom_seq_add_len(prefix, chunk);
    assert((prefix.add(chunk)).subrange(0, prefix.len() as int) =~= prefix) by {
        assert forall|k: int| 0 <= k < prefix.len() implies (prefix.add(chunk)).subrange(
            0,
            prefix.len() as int,
        )[k] == prefix[k] by {
            vstd::seq::axiom_seq_subrange_index(prefix.add(chunk), 0, prefix.len() as int, k);
            vstd::seq::axiom_seq_add_index1(prefix, chunk, k);
        }
    };
}

/// Being an initial segment is transitive.
///
/// The induction uses this once per step to carry the plan correspondence from the
/// step's own post-state back to the prefix the recursive call reasons about.
pub(super) proof fn lemma_extends_transitive(
    a: Seq<Agendum>,
    b: Seq<Agendum>,
    c: Seq<Agendum>,
)
    requires
        spec_extends(a, b),
        spec_extends(b, c),
    ensures spec_extends(a, c),
{
    reveal(spec_extends);
    vstd::seq::axiom_seq_subrange_len(c, 0, a.len() as int);
    assert(c.subrange(0, a.len() as int) =~= a) by {
        assert forall|k: int|
            #![trigger c.subrange(0, a.len() as int)[k]]
            0 <= k < a.len() implies c.subrange(0, a.len() as int)[k] == a[k] by {
            vstd::seq::axiom_seq_subrange_index(c, 0, a.len() as int, k);
            vstd::seq::axiom_seq_subrange_index(b, 0, b.len() as int, k);
            vstd::seq::axiom_seq_subrange_index(b, 0, a.len() as int, k);
        }
    };
}

/// `INV-DRIVER-WRITTEN` — what the fold has established about what it has written.
///
/// Read the conjuncts as three groups.
///
/// The accounting group says the fold has written as many agenda as source lines
/// it has released, that those agenda are the plan's own initial segment, and
/// that the guard `spec_balanced` still holds. The initial-segment conjunct is
/// what ties the per-step decisions to the emission contract: without it a step
/// could only talk about agendum it invented.
///
/// The preservation group says the two runs agree in state over everything
/// written so far. It is at the *written* prefix, not at the fold index, because
/// the buffered lines have not been written yet.
///
/// The span group names the partition the composition joins: a chain whose last
/// cut is the written prefix, whose last state is the source's state there, and
/// which satisfies the composition's own predicate.
///
/// This half of the invariant is what a step that writes anything has to move
/// forward; the buffer half below is what a step that writes nothing has to
/// carry. Splitting them keeps each induction branch about one question.
pub open spec fn spec_fold_written(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    plan_agenda: Seq<Agendum>,
    i: int,
) -> bool {
    let drive = spec_drive(source, rewritable, i);
    let chain = spec_chain_of(source, rewritable, i);
    let last = chain.cuts.len() as int - 1;
    spec_balanced(drive, i) && spec_extends(drive.out, plan_agenda) && spec_state_seeded(
        source,
        drive.out.len() as int,
        None,
    ) == spec_state_seeded(emitted, drive.out.len() as int, None) && chain.cuts.len()
        == chain.states.len() && chain.cuts.len() > 0 && chain.cuts[0] == 0 && chain.states[0]
        == None::<FenceState> && chain.cuts[last] == drive.out.len() as int && chain.states[last]
        == spec_state_seeded(source, drive.out.len() as int, None) && spec_chain_preserved(
        source,
        emitted,
        chain.cuts,
        chain.states,
        last,
    )
}

/// `INV-DRIVER-BUFFER` — what the fold has established about what it is holding.
///
/// The block group is conditional on a block being buffered. Its opener facts
/// ([`spec_masked`] excluded, so the read-back lemma's guard holds), the
/// state-recurrence fact that every line since the opener left the same fence
/// open, and the planner's conflict fold over exactly those lines are the three
/// hypotheses the released-span theorem asks for. None of them is assumed by the
/// released-span theorem itself: they are established here, one step at a time.
///
/// With nothing buffered the fold owes a weaker fact — the tracker is outside
/// every fence — which the next direct write needs.
pub open spec fn spec_fold_buffered(
    source: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    i: int,
) -> bool {
    let drive = spec_drive(source, rewritable, i);
    match drive.state.pending {
        Some(block) => {
            drive.state.start < i && source[drive.state.start].marker == Some(block.opening.marker)
                && source[drive.state.start].marker_len == block.opening.marker_len
                && source[drive.state.start].depth == block.opening.open_depth
                && spec_masked(
                    spec_state_seeded(source, drive.state.start, None),
                    block.opening.open_depth,
                ).is_none() && (forall|j: int|
                #![trigger spec_state_seeded(source, j, None)]
                drive.state.start < j <= i ==> spec_state_seeded(source, j, None) == Some(
                    block.opening,
                )) && block.conflicts == spec_absorb_all(
                spec_fresh(block.opening),
                spec_buffered(source, drive.state.start + 1, i),
                i - drive.state.start - 1,
            ).conflicts
        },
        None => spec_state_seeded(source, i, None).is_none(),
    }
}

/// `INV-DRIVER-FOLD` — everything the fold has established after a prefix.
///
/// The conjunction of the two halves. The document theorem needs both: the
/// written half to join the spans it has completed, the buffer half to describe
/// the block it will release at the end. Stated as one name so the induction's
/// recursive call carries one hypothesis.
pub open spec fn spec_fold_invariant(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    plan_agenda: Seq<Agendum>,
    i: int,
) -> bool {
    spec_fold_written(source, emitted, rewritable, plan_agenda, i) && spec_fold_buffered(
        source,
        rewritable,
        i,
    )
}

} // verus!