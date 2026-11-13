//! The planner's fold over the lines a block buffers, stated as a sequence.
//!
//! The fold's invariant says a buffered block is exactly the absorption of the
//! lines it covers. That absorption is [`spec_absorb_all`], a recursion over a
//! buffer; [`spec_buffered`] is the same buffer spelled as a sequence of source
//! lines, so a lemma can say "the block's conflict flag is the fold over its own
//! interior" and index the result by document position rather than by loop step.
//!
//! Nothing here restates the specification. [`spec_buffered`] is the planner's own
//! fold, and the lemmas below are the two projections out of it: its length, and
//! the identity of its `k`-th entry. They are what let the induction relate the
//! planner's answer for a buffered line to the document's line at the same offset.
//!
//! The transition lemmas — the case splits on [`crate::fence_spec::spec_fence_next`]
//! that turn a routing answer into facts about the line — are in
//! `plan_spec_fold_transition.rs`.

use crate::fence_spec::{spec_compression_changes_region, spec_interior_delimiter};
use crate::fence_spec::spec_rewrite_permitted;
use crate::production_fence::{FenceState, LineFeatures};
use crate::production_plan::Block as PlanBlock;

use super::{spec_absorb, spec_absorb_all, spec_planner_features};

use vstd::prelude::*;

verus! {

/// The planner's features for the lines in `[lo, hi)`, in order.
///
/// This is the sequence the planner absorbs, and stating it as a sequence is what
/// lets a lemma say "the block's conflict flag is the fold over its own interior"
/// without naming the driver's loop. The order is the buffer order, which is the
/// source order: the planner never reorders, so position `k` of the result is
/// source line `lo + k`.
pub open spec fn spec_buffered(
    source: Seq<LineFeatures>,
    lo: int,
    hi: int,
) -> Seq<Option<LineFeatures>>
    decreases hi - lo
{
    if hi <= lo {
        Seq::empty()
    } else {
        spec_buffered(source, lo, hi - 1).push(spec_planner_features(source[hi - 1]))
    }
}

/// The buffered sequence has one entry per covered line.
///
/// The conflict lemma needs this only to know its fold index is in range, but the
/// equality is also the index correspondence the released agenda relies on: the
/// planner's `k`-th absorbed line is the document's `lo + k`-th.
pub(super) proof fn lemma_buffered_length(
    source: Seq<LineFeatures>,
    lo: int,
    hi: int,
)
    requires 0 <= lo <= hi <= source.len(),
    ensures spec_buffered(source, lo, hi).len() == hi - lo,
    decreases hi - lo,
{
    reveal(spec_buffered);
    if lo < hi {
        lemma_buffered_length(source, lo, hi - 1);
    }
}

/// Position `k` of the buffered sequence is the planner's features for line `lo + k`.
///
/// The conflict fold is indexed by buffer position, the driver's state recurrence
/// by document position. This is the bridge: every lemma that reads the fold's
/// answer for "the line the step is holding" converts through here rather than
/// re-deriving the correspondence from the recursion.
pub(super) proof fn lemma_buffered_index(
    source: Seq<LineFeatures>,
    lo: int,
    hi: int,
    k: int,
)
    requires
        0 <= lo <= hi <= source.len(),
        0 <= k < hi - lo,
    ensures spec_buffered(source, lo, hi)[k] == spec_planner_features(source[lo + k]),
    decreases hi - lo,
{
    reveal(spec_buffered);
    if k == hi - lo - 1 {
        lemma_buffered_length(source, lo, hi - 1);
        vstd::seq::axiom_seq_push_index_same(
            spec_buffered(source, lo, hi - 1),
            spec_planner_features(source[hi - 1]),
            k,
        );
    } else {
        lemma_buffered_index(source, lo, hi - 1, k);
        lemma_buffered_length(source, lo, hi - 1);
        vstd::seq::axiom_seq_push_index_different(
            spec_buffered(source, lo, hi - 1),
            spec_planner_features(source[hi - 1]),
            k,
        );
    }
}

/// A block with the given opener and no conflicts so far.
///
/// The planner's [`spec_absorb_all`] carries an opening state through unchanged,
/// so the fold's invariant can state the conflict flag alone as a fold starting
/// from this block. That keeps the invariant about one boolean — the answer the
/// emitter consults — rather than about a record whose other field is already
/// known.
pub open spec fn spec_fresh(opening: FenceState) -> PlanBlock {
    PlanBlock { opening, conflicts: false }
}

/// Absorbing one more line is one more step of the planner's own fold.
///
/// The invariant states a buffered block's conflict flag as
/// [`spec_absorb_all`] over the lines absorbed so far. When a step absorbs one
/// more line, the flag it records must be the same fold over one more line;
/// without this the induction could only say "some block with this flag", which
/// the next step's guard could not read. The buffered sequence and the document
/// are related by [`lemma_buffered_index`].
/// Absorbing a prefix ignores whatever is appended after it.
///
/// The fold counts down from `upto`, reading `lines[upto - 1]` at each step, so its
/// answer for a prefix of `lines` cannot depend on a later entry. The induction
/// states exactly that, and it is what lets the step's one-line extension be
/// compared against the invariant's fold over the shorter buffer.
pub proof fn lemma_absorb_all_push_prefix(
    block: PlanBlock,
    lines: Seq<Option<LineFeatures>>,
    extra: Option<LineFeatures>,
    upto: int,
)
    requires 0 <= upto <= lines.len(),
    ensures spec_absorb_all(block, lines.push(extra), upto) == spec_absorb_all(block, lines, upto),
    decreases upto,
{
    reveal(spec_absorb_all);
    if upto > 0 {
        lemma_absorb_all_push_prefix(block, lines, extra, upto - 1);
        vstd::seq::axiom_seq_push_index_different(lines, extra, upto - 1);
    }
}

pub proof fn lemma_absorb_all_snoc(
    block: PlanBlock,
    source: Seq<LineFeatures>,
    lo: int,
    hi: int,
)
    requires
        0 <= lo < hi <= source.len(),
        block.conflicts == spec_absorb_all(
            spec_fresh(block.opening),
            spec_buffered(source, lo, hi - 1),
            hi - 1 - lo,
        ).conflicts,
    ensures spec_absorb(block, spec_planner_features(source[hi - 1])).conflicts == spec_absorb_all(
        spec_fresh(block.opening),
        spec_buffered(source, lo, hi),
        hi - lo,
    ).conflicts,
{
    let fresh = spec_fresh(block.opening);
    let lin = spec_buffered(source, lo, hi - 1);
    let upto = hi - 1 - lo;
    let last = spec_planner_features(source[hi - 1]);
    reveal(spec_buffered);
    reveal(spec_absorb_all);
    reveal(spec_absorb);
    reveal(spec_fresh);
    lemma_buffered_length(source, lo, hi - 1);
    super::lemma_absorb_all_preserves_opening(fresh, lin, upto);
    assert(spec_buffered(source, lo, hi) == lin.push(last));
    vstd::seq::axiom_seq_push_index_same(lin, last, upto);
    lemma_absorb_all_push_prefix(fresh, lin, last, upto);
    // The fold over the longer buffer reaches the appended line last, and its
    // shorter fold is the invariant's own.
    assert(spec_absorb_all(fresh, lin.push(last), upto + 1) == spec_absorb(
        spec_absorb_all(fresh, lin, upto),
        last,
    ));
    assert(spec_absorb(block, last) == spec_absorb(spec_absorb_all(fresh, lin, upto), last));
}

/// Every line a conflict-free block absorbed is one a rewrite may pass over.
///
/// This is the bridge from the planner's decision to the guard the released-span
/// theorem assumes. The planner marks a block conflicting exactly when some
/// buffered line is interior content in the compression target's marker family;
/// the released-span theorem asks that each interior line be one a rewrite is
/// permitted to pass over. They are the same condition, and
/// `lemma_no_conflict_all_clear` is what says so — but only for the sequence the
/// planner actually folded, which is why the index bridge is needed here as well.
pub proof fn lemma_interior_permitted(
    source: Seq<LineFeatures>,
    block: PlanBlock,
    start: int,
    keep: int,
)
    requires
        0 <= start,
        start + 1 <= keep <= source.len(),
        !block.conflicts,
        block.conflicts == spec_absorb_all(
            spec_fresh(block.opening),
            spec_buffered(source, start + 1, keep),
            keep - start - 1,
        ).conflicts,
    ensures
        forall|j: int|
            #![trigger source[j]]
            start < j < keep ==> spec_rewrite_permitted(block.opening, source[j]),
{
    let fresh = spec_fresh(block.opening);
    let lin = spec_buffered(source, start + 1, keep);
    let upto = keep - start - 1;
    reveal(spec_fresh);
    lemma_buffered_length(source, start + 1, keep);
    super::lemma_no_conflict_all_clear(fresh, lin, upto);
    assert forall|j: int|
        #![trigger source[j]]
        start < j < keep implies spec_rewrite_permitted(block.opening, source[j]) by {
        let k = j - start - 1;
        assert(0 <= k < upto);
        lemma_buffered_index(source, start + 1, keep, k);
        assert(lin[k] == spec_planner_features(source[j]));
        reveal(spec_rewrite_permitted);
        match source[j].marker {
            Some(marker) => {
                reveal(spec_planner_features);
                assert(spec_planner_features(source[j]) == Some(source[j]));
            },
            None => {
                reveal(spec_compression_changes_region);
                reveal(spec_interior_delimiter);
                reveal(spec_planner_features);
            },
        }
    }
}

} // verus!