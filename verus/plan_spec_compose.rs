//! Composition of block results into the document theorem.
//!
//! `verus/plan_spec.rs` proves results about a single released block, stated
//! over a prefix of a source sequence. This file lifts them to whole documents:
//! it shows that a span is itself a document, that a released span reads back
//! into the state it was released from, and that agreement over spans composes
//! into agreement over the document they partition. Those are the lemmas the
//! document theorem applies once per block.
//!
//! This file is the facade. It holds the segment specifications every child
//! works against — [`spec_regions_from`], [`spec_state_from`] — together with
//! the hop from the seeded prefix form to the segment form, the same-reading
//! induction that carries a segment, and the per-span obligation the composition
//! consumes. The children depend on those; none of them is depended on here, so
//! the direction of the dependency is one way and the facade can be read first.
//!
//! The split is by subject rather than by size: [`span`] carries a span to its
//! own lines, [`release`] states which obligation a release owes, and [`chain`]
//! joins the spans of a partition. Selection — which lines a planner buffers,
//! releases and rewrites — is stated in `verus/plan_spec.rs`, and the parent's
//! theorem reduces the pass to a sequence of block results. What happens
//! *between* blocks, and how one block's result chains onto the next, is the
//! subject here.
//!
//! Nothing in this file is reachable from the crate root. The parents of these
//! lemmas are private to `plan_spec`, and the four items they need from it are
//! marked `pub(super)`, so moving them here widens no public surface: the set
//! of names `plan_spec` publishes is the same before and after.
//!
//! Extracted from `verus/plan_spec.rs` so both files come in under the 400-line
//! ceiling `AGENTS.md` sets, and split again into children here for the same
//! reason. The parent declares this file with a private `#[path]` module and
//! reaches every item through a private glob.

use crate::fence_spec::{spec_fence_next, spec_fence_region, spec_regions_seeded, spec_state_seeded};
use crate::production_fence::{FenceState, LineFeatures, Region};
use super::{lemma_same_reading_preserves_next, lemma_same_reading_preserves_region, spec_same_reading};

use vstd::prelude::*;

verus! {

/// Whether a span preserves its regions and lands where the caller says it does.
///
/// This is the per-span obligation the composition consumes, named so the chaining
/// lemma can quantify over spans rather than restate the three equalities at every
/// use. A span satisfies it when both runs read it from the same state, end in the
/// same state, and classify identically throughout.
///
/// Stating the obligation separately is also what makes the composition honest
/// about its own seam: the caller must show this for each span, and the composition
/// shows nothing else. Every way the pass releases a block — a matched close, an
/// abandoned block, the document's final flush — supplies the predicate by citing
/// the block lemma that covers it, so the composition never has to know which kind
/// of release produced the span it is joining.
pub closed spec fn ends_at(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
    target: Option<FenceState>,
) -> bool {
    spec_state_from(source, start, end, seed) == target
        && spec_state_from(emitted, start, end, seed) == target
        && spec_regions_from(source, start, end, seed) == spec_regions_from(
            emitted,
            start,
            end,
            seed,
        )
}

/// The regions of the lines in `[start, end)`, read from the state at `start`.
///
/// [`spec_regions_seeded`] reads a *prefix* from a seed, which is what every
/// induction above wants but not what a document-level argument needs: a document
/// is emitted in segments — a run of directly written lines, then a released block,
/// then another run — and each segment is read from the state the previous one left
/// behind. This specification names a segment directly, so the composition can be
/// a statement about segments rather than about the fold that produced them.
///
/// The state at `start` is *not* recomputed here. Passing it in is deliberate: a
/// caller composing two adjacent segments has to show they agree at the join, and
/// taking the state as a parameter is what makes that obligation visible rather
/// than hidden inside a recursion.
pub open spec fn spec_regions_from(
    features: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
) -> Seq<Region>
    decreases end - start
{
    if end <= start {
        Seq::empty()
    } else {
        spec_regions_from(features, start, end - 1, seed).push(
            spec_fence_region(spec_state_from(features, start, end - 1, seed), features[end - 1]),
        )
    }
}

/// The fence state after the lines in `[start, end)` have been read.
pub open spec fn spec_state_from(
    features: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
) -> Option<FenceState>
    decreases end - start
{
    if end <= start {
        seed
    } else {
        spec_fence_next(
            spec_state_from(features, start, end - 1, seed),
            features[end - 1],
        )
    }
}

/// `LEM-CLASSIFY-FROM-IS-THE-SEEDED-PREFIX` — the two specifications agree at `0`.
///
/// [`spec_regions_seeded`] is the prefix form every earlier lemma is stated over,
/// and [`spec_regions_from`] is the segment form the composition needs. Naming
/// their agreement at the document start is what lets the block lemmas above, which
/// are all in the prefix form, discharge obligations in the segment form.
pub proof fn lemma_classify_from_is_prefix(
    features: Seq<LineFeatures>,
    end: int,
    seed: Option<FenceState>,
)
    requires end <= features.len(),
    ensures
        spec_state_from(features, 0, end, seed) == spec_state_seeded(features, end, seed),
        spec_regions_from(features, 0, end, seed) == spec_regions_seeded(features, end, seed),
    decreases end,
{
    reveal(spec_regions_from);
    reveal(spec_state_from);
    if end > 0 {
        lemma_classify_from_is_prefix(features, end - 1, seed);
    }
}

/// `LEM-AGREEMENT-OVER-A-SEGMENT` — pointwise agreement carries a span.
///
/// [`lemma_agreement_preserves_prefix`] states this for a prefix of the whole
/// document. The composition below needs it for an arbitrary `[start, end)`,
/// because that is the shape a released block has: the lines before the block
/// have already been accounted for, so its argument begins from the state those
/// lines left rather than from the document's seed.
///
/// Nothing in the argument changes with the offset. Each line of the span reads
/// back as itself, so each contributes the same region and leaves the same state,
/// and the two folds therefore end together. The prefix form cannot supply this:
/// it is indexed from zero, and a caller holding a span cannot recover the offset
/// from a statement about the whole document.
///
/// This is the whole argument for every release that writes no compressed
/// delimiter — an abandoned block, and a matched block whose endpoints are not
/// both rewrite candidates. It excludes a conflict-bearing block whose delimiter
/// carries a `null` language specifier: a conflict does make the pass fall back
/// to the source spelling, but that spelling drops the specifier and so raises
/// `trailing_blank`, which this lemma's `spec_same_reading` hypothesis does not
/// admit. See [`lemma_agreement_preserves_prefix`] for the same boundary.
pub proof fn lemma_agreement_from_segment(
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
    ensures
        spec_state_from(source, start, end, seed) == spec_state_from(emitted, start, end, seed),
        spec_regions_from(source, start, end, seed) == spec_regions_from(
            emitted,
            start,
            end,
            seed,
        ),
    decreases end - start,
{
    reveal(spec_regions_from);
    reveal(spec_state_from);
    if start < end {
        lemma_agreement_from_segment(source, emitted, start, end - 1, seed);
        let before = spec_state_from(source, start, end - 1, seed);
        assert(spec_same_reading(source[end - 1], emitted[end - 1]));
        lemma_same_reading_preserves_region(before, source[end - 1], emitted[end - 1]);
        lemma_same_reading_preserves_next(before, source[end - 1], emitted[end - 1]);
    }
}

/// Segment laws: a span read from its own start is a walk over a copy of it.
///
/// Declared here rather than in `verus/lib.rs` so the translation sits beside the
/// segment specifications it converts to, and so the mutation mirror carries the
/// file without a second list to keep in step. The child needs the segment
/// specifications above it, so it is declared after them.
#[path = "plan_spec_compose_span.rs"]
mod span;
// The span laws the composition publishes. `lemma_regions_from_appends` is
// deliberately absent: it was private before the split and stays private, so it
// is re-exported here only to the chain child, not to `plan_spec`. A glob would
// have widened it, because a `pub use` of a visible-in-this-module item is a
// public re-export regardless of the item's own reach.
pub use span::{
    lemma_segment_equals_its_subrange, lemma_segment_reads_from_its_own_seed, lemma_walk_at_offset,
};

/// The released-block route contract: which composition obligation a release owes.
///
/// Declared after its sibling `span`, which supplies the copy-walk lemma that
/// translates a block's own lines to the position it occupies.
#[path = "plan_spec_compose_release.rs"]
mod release;
pub use release::{lemma_released_span_composes, lemma_released_span_reads_back};

/// The composition law: a partition into preserved spans preserves the document.
///
/// Declared after the two children above, whose results it joins.
#[path = "plan_spec_compose_chain.rs"]
mod chain;
pub use chain::{
    lemma_agreement_span_is_preserved, lemma_prefix_splits_at_seeded_state,
    lemma_spans_compose_regions, spec_chain_preserved,
};

} // verus!
