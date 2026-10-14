//! The composition law: a partition into preserved spans preserves the document.
//!
//! The pass never reasons about a whole document at once. It reads a prefix,
//! buffers the block it is inside, releases that block as one span, and carries
//! on, so every conclusion it needs is about a span and the document result is
//! those span results joined end to end. This file states the predicate that
//! chains the spans, the lemma that joins them, and the bridge that lets a span
//! lying inside a prefix be read on its own.
//!
//! The predicate is a recursion rather than a quantifier: a quantified `ends_at`
//! puts a recursive specification at a symbolic index into every solver query, so
//! the verifier cannot decide whether to unfold it. Peeling one cut per level at
//! concrete indices is what a proof of this size can afford.
//!
//! The child is declared by `verus/plan_spec_compose.rs`; it depends on the
//! facade for the span predicate and segment specifications, and on its sibling
//! `compose_span` for the segmentation law the join applies at each cut.

use crate::fence_spec::{spec_regions_seeded, spec_state_seeded};
use crate::production_fence::{FenceState, LineFeatures, Region};


use super::super::{ spec_same_reading };
use super::{ ends_at, lemma_agreement_from_segment, lemma_classify_from_is_prefix, spec_regions_from, spec_state_from };

use super::span::lemma_regions_from_appends;

use vstd::prelude::*;

verus! {

/// `LEM-AGREEMENT-SPAN-IS-PRESERVED` — the predicate follows from pointwise agreement.
///
/// This is how the composition discharges the spans the pass writes through without
/// compressing anything: an abandoned block, the run of lines between two blocks, a
/// matched block whose endpoints are not both rewrite candidates, and every
/// conflict-bearing block, whose fallback writes the source spelling at both ends.
/// Pointwise agreement is the only hypothesis any of them needs, and
/// [`lemma_agreement_from_segment`] is the induction that supplies it.
pub proof fn lemma_agreement_span_is_preserved(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
)
    requires
        emitted.len() == source.len(),
        0 <= start <= end <= source.len(),
        forall|i: int|
            #![trigger source[i]]
            start <= i < end ==> spec_same_reading(source[i], #[trigger] emitted[i]),
    ensures ends_at(source, emitted, start, end, seed, spec_state_from(source, start, end, seed)),
{
    reveal(ends_at);
    lemma_agreement_from_segment(source, emitted, start, end, seed);
}

/// Whether every span below `t` preserves its regions and lands where it should.
///
/// This is the composition's hypothesis as a single term rather than a quantifier.
/// The recursion peels the *last* span, matching the direction the composition
/// walks: the caller has a document cut into spans and joins them from the front,
/// so the fact each step consumes is the fact about the highest remaining cut.
///
/// The ordering of the cuts is part of the predicate, not a separate hypothesis,
/// because a span whose end is before its start is not a span. Carrying the
/// ordering here means each induction step gets both facts about its last span from
/// one unfold, and means the bounds the split law needs at the joint are available
/// without a second chain of arithmetic.
///
/// A quantifier over the cut sequence would say the same thing, and it is what the
/// first attempt used. It is the wrong shape here. A quantified `ends_at` puts a
/// recursive spec function at a symbolic index into every solver query, so the
/// verifier cannot decide whether to unfold it and spends its budget trying. A
/// recursive predicate is instead reduced one level per call at concrete indices,
/// which is what a proof of this size can afford.
pub open spec fn spec_chain_preserved(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    cuts: Seq<int>,
    states: Seq<Option<FenceState>>,
    t: int,
) -> bool
    decreases t
{
    if t <= 0 {
        0 <= cuts[0] <= source.len()
    } else {
        0 <= cuts[t - 1] <= cuts[t] && cuts[t] <= source.len() && ends_at(
            source,
            emitted,
            cuts[t - 1],
            cuts[t],
            states[t - 1],
            states[t],
        ) && spec_chain_preserved(source, emitted, cuts, states, t - 1)
    }
}

/// `LEM-PREFIX-SPLITS-AT-ITS-SEEDED-STATE` — a prefix is its head then its tail.
///
/// The split law says the same thing, but spelled with the segment form on the
/// right. This is the spelling the composition uses at every join: the document so
/// far, then the span currently being emitted, read from the state the document so
/// far left. Stating it once keeps the composition from re-deriving the three
/// conversions at each of its cases.
pub proof fn lemma_prefix_splits_at_seeded_state(
    features: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
)
    requires 0 <= start <= end <= features.len(),
    ensures
        spec_state_seeded(features, end, seed) == spec_state_from(
            features,
            start,
            end,
            spec_state_seeded(features, start, seed),
        ),
        spec_regions_seeded(features, end, seed) == spec_regions_seeded(
            features,
            start,
            seed,
        ).add(spec_regions_from(features, start, end, spec_state_seeded(features, start, seed))),
{
    lemma_classify_from_is_prefix(features, start, seed);
    lemma_classify_from_is_prefix(features, end, seed);
    lemma_regions_from_appends(features, 0, start, end, seed);
}

/// `LEM-SPANS-COMPOSE` — a partition into preserved spans preserves the document.
///
/// This is the composition law, and it is the statement the driver's fold
/// discharges. The pass never reasons about a whole document at once: it reads a
/// prefix, buffers the block it is inside, releases that block as one span when a
/// delimiter or a depth change ends it, and carries on. Every conclusion it needs
/// is therefore about a *span* — one released block's lines, or a run of lines
/// written through without buffering — and the document result is those span
/// results joined end to end.
///
/// The hypothesis is deliberately stated over a caller-supplied sequence of
/// states rather than over the seeded folds. That is what makes the lemma a
/// composition instead of a restatement: each span is read from a state the caller
/// names, the same state for the source run and the emitted run, so the two runs
/// are compared span by span without either side having to unfold the other's
/// fold. The state and region equalities the lemma concludes are then exactly what
/// the next span needs as its seed, which is why the caller can walk a document's
/// cut points left to right and never revisit one.
///
/// The induction removes the last span, which is the direction
/// [`spec_chain_preserved`] peels, so the hypothesis at the shorter chain is the
/// hypothesis at the longer one definitionally rather than by another induction.
pub proof fn lemma_spans_compose_regions(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    cuts: Seq<int>,
    states: Seq<Option<FenceState>>,
    k: int,
)
    requires
        emitted.len() == source.len(),
        cuts.len() > k,
        states.len() > k,
        0 <= k,
        cuts[0] == 0,
        spec_chain_preserved(source, emitted, cuts, states, k),
    ensures
        spec_state_seeded(source, cuts[k], states[0]) == states[k],
        spec_state_seeded(emitted, cuts[k], states[0]) == states[k],
        spec_regions_seeded(source, cuts[k], states[0]) == spec_regions_seeded(
            emitted,
            cuts[k],
            states[0],
        ),
    decreases k,
{
    reveal(spec_chain_preserved);
    if k == 0 {
        // The empty partition is the document that is a single point: no span at
        // all, so the state is the seed and the regions are empty. The base case
        // is not decorative — it is what makes the induction below total, and it
        // is the shape the caller gets for a document whose whole content was
        // written through without ever buffering a block.
        assert(spec_state_seeded(source, 0, states[0]) == states[0]) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        assert(spec_regions_seeded(source, 0, states[0]) == Seq::<Region>::empty()) by {
            reveal_with_fuel(spec_regions_seeded, 2);
        };
        assert(spec_state_seeded(emitted, 0, states[0]) == states[0]) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        assert(spec_regions_seeded(emitted, 0, states[0]) == Seq::<Region>::empty()) by {
            reveal_with_fuel(spec_regions_seeded, 2);
        };
    } else {
        // The induction is over the number of spans and removes the last one, so
        // the shorter chain is exactly the spans this step does not touch. Naming
        // the two cut bounds as locals before the call lets the arithmetic that
        // follows read them rather than the sequence indexing.
        let joint = cuts[k - 1];
        let last = cuts[k];
        assert(spec_chain_preserved(source, emitted, cuts, states, k - 1));
        lemma_spans_compose_regions(source, emitted, cuts, states, k - 1);
        // The last span's contribution, read out of the hypothesis. Unfolding the
        // chain at `k` splits it into the last span's fact and the shorter chain,
        // which is exactly the hypothesis the recursive call consumed.
        assert(ends_at(source, emitted, joint, last, states[k - 1], states[k]));
        reveal(ends_at);
        assert(spec_state_from(source, joint, last, states[k - 1]) == states[k]);
        assert(spec_state_from(emitted, joint, last, states[k - 1]) == states[k]);
        assert(spec_regions_from(source, joint, last, states[k - 1]) == spec_regions_from(
            emitted,
            joint,
            last,
            states[k - 1],
        ));
        // The prefix's result is the seed the last span is read from, which the
        // induction supplies for both runs at once.
        assert(spec_state_seeded(source, joint, states[0]) == states[k - 1]) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        assert(spec_state_seeded(emitted, joint, states[0]) == states[k - 1]) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        // Joining the two halves is the split law at the joint, not a second
        // induction: the prefix and the last span are read consecutively from the
        // same state, so their regions concatenate.
        lemma_prefix_splits_at_seeded_state(source, joint, last, states[0]);
        lemma_prefix_splits_at_seeded_state(emitted, joint, last, states[0]);
        assert(spec_state_seeded(source, last, states[0]) == states[k]);
        assert(spec_state_seeded(emitted, last, states[0]) == states[k]);
        assert(spec_regions_seeded(source, last, states[0]) == spec_regions_seeded(
            source,
            joint,
            states[0],
        ).add(spec_regions_from(source, joint, last, states[k - 1])));
        assert(spec_regions_seeded(emitted, last, states[0]) == spec_regions_seeded(
            emitted,
            joint,
            states[0],
        ).add(spec_regions_from(emitted, joint, last, states[k - 1])));
    }
}

} // verus!
