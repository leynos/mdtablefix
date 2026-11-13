//! How one fold step moves the written half of the invariant.
//!
//! The induction in `plan_spec_fold_regions.rs` splits on whether the step writes
//! anything. Both answers end in the same three obligations — the length guard,
//! the plan correspondence, and the span chain — and differ only in whether the
//! chain gains a cut. These two lemmas are that pair, so each route of the
//! induction states its span fact and nothing else.
//!
//! Keeping the pair here rather than in the induction is a matter of shape: the
//! step's route determines *which* span fact it can supply (a released block, a
//! directly written line, or nothing), but the way a span fact moves the chain
//! forward is the same in every route.

use crate::fence_spec::spec_state_seeded;
use crate::production_fence::LineFeatures;
use crate::production_plan::Agendum;

use super::{
    ends_at, lemma_chain_preserved_push, spec_balanced, spec_chain_advance, spec_chain_of,
    spec_chain_preserved, spec_drive, spec_extends, spec_fold_written,
};

use vstd::prelude::*;

verus! {

/// A step that writes nothing leaves the written half of the invariant alone.
///
/// An `Extend` buffers a line and a restart with nothing pending opens a block;
/// neither releases an agendum, so the spans completed so far are unchanged. The
/// written prefix is unchanged too, which is why the preservation and chain
/// conjuncts carry over verbatim and the two facts about the current prefix —
/// the length guard and the plan correspondence — are the only new inputs.
pub proof fn lemma_written_static(
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
        spec_fold_written(source, emitted, rewritable, plan_agenda, i - 1),
        spec_drive(source, rewritable, i).out.len() == spec_drive(source, rewritable, i - 1).out.len(),
        spec_balanced(spec_drive(source, rewritable, i), i),
        spec_extends(spec_drive(source, rewritable, i).out, plan_agenda),
    ensures spec_fold_written(source, emitted, rewritable, plan_agenda, i),
{
    reveal(spec_fold_written);
    reveal(spec_chain_of);
}

/// A step that writes a span appends one cut to the chain.
///
/// The span's own preservation is the caller's business — this lemma does not
/// care whether it came from a released block or from a directly written line,
/// only that it is a span from the old written prefix to the new one, read from
/// the source state at the joint. That is exactly the shape
/// [`spec_chain_preserved`] peels, so the new last span is appended and the rest
/// of the chain is the recursive hypothesis.
pub proof fn lemma_written_extends(
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
        spec_fold_written(source, emitted, rewritable, plan_agenda, i - 1),
        spec_drive(source, rewritable, i).out.len() > spec_drive(
            source,
            rewritable,
            i - 1,
        ).out.len(),
        spec_balanced(spec_drive(source, rewritable, i), i),
        spec_extends(spec_drive(source, rewritable, i).out, plan_agenda),
        ends_at(
            source,
            emitted,
            spec_drive(source, rewritable, i - 1).out.len() as int,
            spec_drive(source, rewritable, i).out.len() as int,
            spec_state_seeded(
                source,
                spec_drive(source, rewritable, i - 1).out.len() as int,
                None,
            ),
            spec_state_seeded(source, spec_drive(source, rewritable, i).out.len() as int, None),
        ),
        spec_state_seeded(source, spec_drive(source, rewritable, i).out.len() as int, None)
            == spec_state_seeded(
                emitted,
                spec_drive(source, rewritable, i).out.len() as int,
                None,
            ),
    ensures spec_fold_written(source, emitted, rewritable, plan_agenda, i),
{
    let prev = spec_drive(source, rewritable, i - 1);
    let next = spec_drive(source, rewritable, i);
    let before = spec_chain_of(source, rewritable, i - 1);
    let after = spec_chain_of(source, rewritable, i);
    let last = before.cuts.len() as int - 1;
    reveal(spec_fold_written);
    reveal(spec_chain_of);
    reveal(spec_chain_advance);
    reveal(spec_chain_preserved);
    // The recursion appends exactly when the prefix's written length grew, which
    // is the hypothesis above; after the append the new last cut is the index the
    // push wrote, and the cut before it is the old last cut.
    assert(after == spec_chain_advance(
        before,
        next.out.len() as int,
        spec_state_seeded(source, next.out.len() as int, None),
    ));
    reveal(spec_chain_advance);
    assert(after.cuts == before.cuts.push(next.out.len() as int));
    assert(after.states == before.states.push(spec_state_seeded(
        source,
        next.out.len() as int,
        None,
    )));
    vstd::seq::axiom_seq_push_len(before.cuts, next.out.len() as int);
    vstd::seq::axiom_seq_push_len(before.states, spec_state_seeded(
        source,
        next.out.len() as int,
        None,
    ));
    assert(after.cuts.len() == before.cuts.len() + 1);
    assert(after.states.len() == before.states.len() + 1);
    assert(before.cuts.len() == before.states.len());
    assert(after.cuts.len() == after.states.len());
    vstd::seq::axiom_seq_push_index_same(before.cuts, next.out.len() as int, last + 1);
    vstd::seq::axiom_seq_push_index_same(before.states, spec_state_seeded(
        source,
        next.out.len() as int,
        None,
    ), last + 1);
    vstd::seq::axiom_seq_push_index_different(before.cuts, next.out.len() as int, 0);
    vstd::seq::axiom_seq_push_index_different(before.states, spec_state_seeded(
        source,
        next.out.len() as int,
        None,
    ), 0);
    vstd::seq::axiom_seq_push_index_different(before.cuts, next.out.len() as int, last);
    vstd::seq::axiom_seq_push_index_different(before.states, spec_state_seeded(
        source,
        next.out.len() as int,
        None,
    ), last);
    assert(after.cuts.len() as int == last + 2);
    assert(after.cuts[last + 1] == next.out.len() as int);
    assert(after.states[last + 1] == spec_state_seeded(source, next.out.len() as int, None));
    assert(after.cuts[last] == prev.out.len() as int);
    assert(after.states[last] == spec_state_seeded(source, prev.out.len() as int, None));
    // The new top span is the caller's ends-at fact, and every span below it is
    // the chain the shorter prefix already accumulated.
    assert(next.out.len() <= source.len());
    lemma_chain_preserved_push(
        source,
        emitted,
        before.cuts,
        before.states,
        last,
        next.out.len() as int,
        spec_state_seeded(source, next.out.len() as int, None),
    );
}

} // verus!