//! Segment laws: a span read from its own start is a walk over a copy of it.
//!
//! The block lemmas the composition consumes are all stated over a prefix of a
//! sequence read from position zero, because that is the only way their
//! inductions close. A released block sits in the middle of a document, so the
//! composition needs the translation from "a prefix of this sequence" to "this
//! span, read from where the previous span left off". These four lemmas are that
//! translation, and the segmentation law the facade's segment specifications
//! ([`crate::spec_regions_from`] and its state sibling) rest on.
//!
//! The child is declared by `verus/plan_spec_compose.rs`, which supplies the
//! segment specifications and the prefix bridge; this file depends on the facade
//! rather than the other way round, so the dependency runs one way.

use crate::fence_spec::{spec_fence_region, spec_regions_seeded, spec_state_seeded};
use crate::production_fence::{FenceState, LineFeatures};
use vstd::seq::axiom_seq_subrange_index;


use super::{ lemma_classify_from_is_prefix, spec_regions_from, spec_state_from };

use vstd::prelude::*;

verus! {

// ---------------------------------------------------------------------------

/// `LEM-SEGMENT-EQUALS-ITS-SUBRANGE` — a span is a document of its own.
///
/// This is the lemma that makes every prefix-stated block result usable at any
/// offset, and it is the cheapest possible form of that translation: take the
/// lines the span covers as a sequence in their own right, and the two folds
/// agree on it, because a fold from `start` walking up to `end` visits exactly the
/// lines a fold from `0` walking up to `end - start` visits — the same line at each
/// step, by `axiom_seq_subrange_index`, and therefore the same region and the same
/// state.
///
/// With this, a block lemma stated over `[0, len)` — and all of them are, because
/// the pass's first block starts at zero — applies to a block beginning anywhere,
/// by reading the block's lines as the subrange they are. That is what the
/// composition needs and it costs one induction rather than a second copy of the
/// compressed-block argument at every offset.
pub proof fn lemma_segment_equals_its_subrange(
    features: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
)
    requires 0 <= start <= end <= features.len(),
    ensures
        spec_state_from(features, start, end, seed) == spec_state_seeded(
            features.subrange(start, end),
            end - start,
            seed,
        ),
        spec_regions_from(features, start, end, seed) == spec_regions_seeded(
            features.subrange(start, end),
            end - start,
            seed,
        ),
    decreases end - start,
{
    reveal(spec_regions_from);
    reveal(spec_state_from);
    reveal(spec_state_seeded);
    reveal(spec_regions_seeded);
    let sub = features.subrange(start, end);
    // The hypothesis the walk lemma takes as given, discharged here from the one
    // vstd axiom that relates a subrange's elements to the original's. This is the
    // only place subrange arithmetic is needed, and it is where the copy is made.
    assert forall|i: int| #![trigger sub[i]] 0 <= i < end - start implies sub[i]
        == features[start + i] by {
        axiom_seq_subrange_index(features, start, end, i);
    };
    lemma_walk_at_offset(features, sub, start, end - start, seed);
}

/// `LEM-WALK-AT-OFFSET` — a walk from an offset is the same walk over a copy.
///
/// The line correspondence is a hypothesis rather than a subrange equation, and
/// that is the whole point: the two folds agree for the plainest possible reason,
/// since at each step they read the same line and apply the same transition. By
/// taking the correspondence as given, this lemma carries no subrange arithmetic of
/// its own — a caller that holds the copy as a genuine subrange discharges the
/// hypothesis with `axiom_seq_subrange_index` and nothing else.
///
/// Stating it this way also keeps the induction honest about what it assumes:
/// `upto` lines of `sub` are related to the `upto` lines of `features` that begin
/// at `start`, and every step peels one pair off that correspondence.
pub proof fn lemma_walk_at_offset(
    features: Seq<LineFeatures>,
    sub: Seq<LineFeatures>,
    start: int,
    upto: int,
    seed: Option<FenceState>,
)
    requires
        0 <= start,
        0 <= upto,
        start + upto <= features.len(),
        upto <= sub.len(),
        forall|i: int| #![trigger sub[i]] 0 <= i < upto ==> sub[i] == features[start + i],
    ensures
        spec_state_from(features, start, start + upto, seed) == spec_state_seeded(sub, upto, seed),
        spec_regions_from(features, start, start + upto, seed) == spec_regions_seeded(
            sub,
            upto,
            seed,
        ),
    decreases upto,
{
    reveal(spec_state_from);
    reveal(spec_regions_from);
    reveal(spec_state_seeded);
    reveal(spec_regions_seeded);
    if upto > 0 {
        // The shorter hypothesis is about the same correspondence, one pair
        // shorter, so the step never has to reconstruct an index relation.
        lemma_walk_at_offset(features, sub, start, upto - 1, seed);
        assert(sub[upto - 1] == features[start + upto - 1]);
    }
}

/// `LEM-SEGMENT-READS-FROM-ITS-OWN-SEED` — a span from its own start is the same walk.
///
/// Every block lemma is stated from the document's start, because that is where
/// the pass begins. A block in the middle of a document is not a prefix: the lines
/// before it are already accounted for and the lines after it have not been read.
/// What the composition needs is the licence to read that span *from its own first
/// line*, seeded with the state the lines before it left — which is precisely what
/// the pass does, since it buffers each block's lines and reads them against the
/// tracker state at the block's opener.
///
/// The result is the split law read at both ends rather than a fresh induction.
/// Reading `[0, end)` in one go is the same as reading `[0, start)` and then
/// `[start, end)` from where the first part ended, so the middle segment's regions
/// are the tail of the whole prefix's — and by the bridge above, the three states
/// involved are the seeded ones.
pub proof fn lemma_segment_reads_from_its_own_seed(
    features: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
)
    requires 0 <= start <= end <= features.len(),
    ensures
        spec_state_from(features, start, end, spec_state_seeded(features, start, seed))
            == spec_state_seeded(features, end, seed),
        spec_regions_from(features, start, end, spec_state_seeded(features, start, seed))
            == spec_regions_seeded(features, end, seed).subrange(
                spec_regions_seeded(features, start, seed).len() as int,
                spec_regions_seeded(features, end, seed).len() as int,
            ),
{
    lemma_classify_from_is_prefix(features, start, seed);
    lemma_classify_from_is_prefix(features, end, seed);
    lemma_regions_from_appends(features, 0, start, end, seed);
    let head = spec_regions_seeded(features, start, seed);
    let full = spec_regions_seeded(features, end, seed);
    let seg = spec_regions_from(features, start, end, spec_state_seeded(features, start, seed));
    // The split law, in the segment form, is about the region sequence cut at the
    // state the first part left. Both conversions are the bridge above: the
    // zero-indexed folds are the seeded ones, and the middle seed is the seeded
    // state at `start`.
    assert(full == head.add(seg));
    // Cutting a concatenation at the joint leaves the second part. Stated with the
    // subrange form the conclusion uses rather than with `skip`, because the two
    // are the same operation written two ways.
    assert(head.add(seg).subrange(head.len() as int, head.add(seg).len() as int) =~= seg) by {
        assert forall|i: int|
            #![trigger head.add(seg).subrange(
                head.len() as int,
                head.add(seg).len() as int,
            )[i]]
            0 <= i < seg.len() implies head.add(seg).subrange(
            head.len() as int,
            head.add(seg).len() as int,
        )[i] == seg[i] by {
            assert(head.add(seg)[i + head.len() as int] == seg[i]);
        }
    };
}

/// `LEM-REGIONS-FROM-APPENDS` — a segment is its first part then its second.
///
/// This is the law that makes segmentation possible: reading `[start, end)` in one
/// go is the same as reading `[start, mid)` and then `[mid, end)` from the state the
/// first part left, for both the regions and the state. Without it, a document
/// argument would have to unfold the whole recursion at every join.
pub(super) proof fn lemma_regions_from_appends(
    features: Seq<LineFeatures>,
    start: int,
    mid: int,
    end: int,
    seed: Option<FenceState>,
)
    requires
        start <= mid <= end,
        end <= features.len(),
    ensures
        spec_state_from(features, start, end, seed) == spec_state_from(
            features,
            mid,
            end,
            spec_state_from(features, start, mid, seed),
        ),
        spec_regions_from(features, start, end, seed) == spec_regions_from(
            features,
            start,
            mid,
            seed,
        ).add(spec_regions_from(
            features,
            mid,
            end,
            spec_state_from(features, start, mid, seed),
        )),
    decreases end - mid,
{
    reveal(spec_regions_from);
    reveal(spec_state_from);
    if mid < end {
        lemma_regions_from_appends(features, start, mid, end - 1, seed);
        // The induction is on the *tail*'s length, so both segments lose their last
        // line together and the shorter hypothesis is about exactly the two prefixes
        // this step needs. Naming the state each side ends in is what lets the last
        // region of either be written against the same state.
        let outer = spec_state_from(features, start, end - 1, seed);
        let inner = spec_state_from(
            features,
            mid,
            end - 1,
            spec_state_from(features, start, mid, seed),
        );
        let head = spec_regions_from(features, start, mid, seed);
        assert(outer == inner);
        assert(spec_regions_from(features, start, end, seed) == head.add(
            spec_regions_from(features, mid, end - 1, spec_state_from(features, start, mid, seed)),
        ).push(spec_fence_region(outer, features[end - 1])));
        assert(spec_regions_from(
            features,
            mid,
            end,
            spec_state_from(features, start, mid, seed),
        ) == spec_regions_from(
            features,
            mid,
            end - 1,
            spec_state_from(features, start, mid, seed),
        ).push(spec_fence_region(inner, features[end - 1])));
        // The appendix moves across the concatenation: the last region belongs after
        // both parts, not after the first.
        Seq::push_distributes_over_add(
            head,
            spec_regions_from(features, mid, end - 1, spec_state_from(features, start, mid, seed)),
            spec_fence_region(outer, features[end - 1]),
        );
    }
}

} // verus!
