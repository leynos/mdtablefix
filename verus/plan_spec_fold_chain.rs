//! The completed spans of a prefix, as the composition law consumes them.
//!
//! [`super::spec_chain_preserved`] is the composition's hypothesis: a partition
//! of the document into spans, each of which preserves its regions and lands in
//! the state the caller names. `verus/plan_spec_compose_chain.rs` proves that such
//! a partition preserves the document. What it does not do is produce the
//! partition, because the partition is a fact about *this* pass: it is the
//! sequence of chunks the driver has released so far.
//!
//! [`spec_chain_of`] is that sequence. It is a recursion over the same prefix the
//! driver folds, and it appends a cut exactly when the fold writes something.
//! That is the whole correspondence: a buffered line belongs to the pending
//! block, a written line or a released block ends a span. The cut's state is the
//! tracker's state at that cut, read from the source — the state the composition
//! compares the emitted run against.
//!
//! Keeping the chain a separate recursion rather than a field of the driver is
//! deliberate. The driver's state is what the production kernel computes, and
//! adding a sequence of cuts to it would put proof bookkeeping into the value the
//! kernel must realise. The chain is a specification of the same decisions, read
//! off the fold after the fact.

use crate::fence_spec::spec_state_seeded;
use crate::production_fence::{FenceState, LineFeatures};

use super::{ends_at, spec_chain_preserved, spec_drive};

use vstd::prelude::*;

verus! {

/// A partition's cut points and the state each cut is read from.
///
/// The two sequences are parallel: `states[t]` is the tracker state after the
/// lines before `cuts[t]`, and the span between two consecutive cuts is one
/// released chunk. `cuts[0]` is always zero and `states[0]` is always the
/// document's seed, which is what makes the first span start at the beginning.
pub struct Chain {
    /// Positions in the document between spans, ascending from zero.
    pub cuts: Seq<int>,
    /// The source state at each cut.
    pub states: Seq<Option<FenceState>>,
}

/// `chain` with one more cut at the end.
pub open spec fn spec_chain_advance(
    chain: Chain,
    cut: int,
    state: Option<FenceState>,
) -> Chain {
    Chain { cuts: chain.cuts.push(cut), states: chain.states.push(state) }
}

/// The spans the driver has completed after a prefix.
///
/// A step that writes nothing — an `Extend`, which only buffers — leaves the
/// chain alone, because its line belongs to the pending block and the block has
/// not been released. Every other step writes a nonempty chunk, and the cut is
/// placed after that chunk with the state the source's own transition leaves
/// there. Reading the state from [`spec_state_seeded`] rather than from the
/// driver is what keeps the chain about the document: the emitted run is compared
/// against the source at the same cut, so the source is the authority.
pub open spec fn spec_chain_of(
    source: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    i: int,
) -> Chain
    decreases i
{
    if i <= 0 {
        Chain { cuts: seq![0], states: seq![None] }
    } else {
        let chain = spec_chain_of(source, rewritable, i - 1);
        let prev = spec_drive(source, rewritable, i - 1);
        let next = spec_drive(source, rewritable, i);
        if next.out.len() == prev.out.len() {
            chain
        } else {
            spec_chain_advance(
                chain,
                next.out.len() as int,
                spec_state_seeded(source, next.out.len() as int, None),
            )
        }
    }
}

/// A chain whose entries are unchanged below a cut preserves the same spans.
///
/// [`spec_chain_preserved`] peels its last span, so extending a chain at the end
/// must leave every span below the old top true without replaying the chain. That
/// is a statement about the entries rather than about the spans: two chains that
/// agree at every index up to `t` satisfy the same predicate at `t`. The induction
/// carries the agreement down one index at a time, which is exactly how many
/// unfolds the predicate needs.
pub(super) proof fn lemma_chain_preserved_prefix(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    cuts: Seq<int>,
    states: Seq<Option<FenceState>>,
    other_cuts: Seq<int>,
    other_states: Seq<Option<FenceState>>,
    t: int,
)
    requires
        0 <= t < cuts.len(),
        0 <= t < other_cuts.len(),
        spec_chain_preserved(source, emitted, cuts, states, t),
        forall|k: int|
            #![trigger cuts[k]]
            0 <= k <= t ==> other_cuts[k] == cuts[k] && other_states[k] == states[k],
    ensures spec_chain_preserved(source, emitted, other_cuts, other_states, t),
    decreases t,
{
    reveal(spec_chain_preserved);
    if t > 0 {
        assert forall|k: int|
            #![trigger cuts[k]]
            0 <= k <= t - 1 implies other_cuts[k] == cuts[k] && other_states[k] == states[k] by {
        }
        lemma_chain_preserved_prefix(source, emitted, cuts, states, other_cuts, other_states, t - 1);
    }
}

/// Appending one preserved span extends the chain predicate by that span.
///
/// A step that writes a chunk releases a span, and the chain gains its cut. The new
/// top span is the step's own ends-at fact; everything below it is the old chain,
/// which agrees with the new one at every remaining index because a push changes
/// only the top. This is the lemma the progress induction applies once per written
/// step, and it is why that induction never has to re-derive the spans it already
/// closed.
pub(super) proof fn lemma_chain_preserved_push(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    cuts: Seq<int>,
    states: Seq<Option<FenceState>>,
    t: int,
    next_cut: int,
    next_state: Option<FenceState>,
)
    requires
        // The new cut extends the chain at its top, which is the only way the
        // predicate's index and the pushed index are the same one.
        0 <= t,
        cuts.len() == t + 1,
        cuts.len() == states.len(),
        spec_chain_preserved(source, emitted, cuts, states, t),
        cuts[t] <= next_cut <= source.len(),
        ends_at(source, emitted, cuts[t], next_cut, states[t], next_state),
    ensures spec_chain_preserved(
        source,
        emitted,
        cuts.push(next_cut),
        states.push(next_state),
        t + 1,
    ),
{
    let cuts2 = cuts.push(next_cut);
    let states2 = states.push(next_state);
    reveal(spec_chain_preserved);
    vstd::seq::axiom_seq_push_len(cuts, next_cut);
    vstd::seq::axiom_seq_push_len(states, next_state);
    vstd::seq::axiom_seq_push_index_same(cuts, next_cut, t + 1);
    vstd::seq::axiom_seq_push_index_same(states, next_state, t + 1);
    assert forall|k: int|
        #![trigger cuts[k]]
        0 <= k < cuts.len() implies cuts2[k] == cuts[k] && states2[k] == states[k] by {
        assert(0 <= k < cuts.len());
        vstd::seq::axiom_seq_push_index_different(cuts, next_cut, k);
        vstd::seq::axiom_seq_push_index_different(states, next_state, k);
    }
    lemma_chain_preserved_prefix(source, emitted, cuts, states, cuts2, states2, t);
}

} // verus!