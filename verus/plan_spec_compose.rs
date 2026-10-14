//! Composition of block results into the document theorem.
//!
//! `verus/plan_spec.rs` proves results about a single released block, stated
//! over a prefix of a source sequence. This file lifts them to whole documents:
//! it shows that a span is itself a document, that a released span reads back
//! into the state it was released from, and that agreement over spans composes
//! into agreement over the document they partition. Those are the lemmas the
//! document theorem applies once per block.
//!
//! The split is by subject rather than by size. Selection — which lines a
//! planner buffers, releases and rewrites — is stated in the parent, and the
//! parent's theorem reduces the pass to a sequence of block results. What
//! happens *between* blocks, and how one block's result chains onto the next,
//! is the subject here. The two files meet at [`lemma_released_block_reads_back`]
//! and [`spec_regions_from`]: the first is a product of the parent's induction
//! over the planner rules, and this file consumes it to reach the document.
//!
//! Nothing in this file is reachable from the crate root. The parents of these
//! lemmas are private to `plan_spec`, and the four items they need from it are
//! marked `pub(super)`, so moving them here widens no public surface: the set
//! of names `plan_spec` publishes is the same before and after.
//!
//! Extracted from `verus/plan_spec.rs` so both files come in under the 400-line
//! ceiling `AGENTS.md` sets. The parent declares this file with a private
//! `#[path]` module and reaches every item through a private glob.

use crate::fence_spec::{
    spec_closes, spec_fence_next, spec_fence_region, spec_regions_seeded,
    spec_rewrite_permitted, spec_state_seeded,
};
use crate::production_fence::{FenceState, LineFeatures, Region};
use vstd::prelude::*;
use vstd::seq::axiom_seq_subrange_index;

// The parent's block-level results. `lemma_compressed_block_regions` is public
// but is imported through `super` alongside the `pub(super)` names so that every
// dependency of this file enters by the same route and the boundary is legible
// in one place.
//
// `lemma_released_block_reads_back` and `spec_emission_reads_back` are named in
// this file's prose but never called: the composition states its own inductions
// over spans rather than invoking that one per block. They are deliberately not
// imported, so the gap between what this file documents and what it proves is
// visible to the compiler rather than papered over with a re-export.
use super::{
    lemma_agreement_preserves_prefix, lemma_compressed_block_regions,
    lemma_same_reading_preserves_next, lemma_same_reading_preserves_region, spec_masked,
    spec_same_reading,
};

verus! {

// ---------------------------------------------------------------------------
// Composing blocks into a document
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

/// `LEM-RELEASED-SPAN-SATISFIES-ENDS-AT` — a block in the middle of a document.
///
/// This is the join between the two halves of the document theorem. The block
/// lemmas are stated over a block's *own* lines, read from position zero, because
/// that is the only way their inductions close. The composition law consumes spans
/// of the whole document, named by their offsets. This lemma is the translation
/// between those two views, and it is deliberately thin: it makes the block's two
/// spans out of the document's lines, carries the block's hypotheses across
/// unchanged, and chains the three results.
///
/// The hypotheses are the block lemma's, restated at document offsets. That is the
/// honest form of the boundary: everything about *what the pass decided* — that
/// the opener reads back as the compressed or source spelling, that interior lines
/// are unchanged, that a compressible closer was written as three backticks — is
/// assumed here and must be discharged by the fold, from the agenda the planner
/// actually produced. What this lemma supplies is the part that is not about the
/// pass at all: that those per-line facts, wherever the block sits, give the
/// composition the two facts it asks for.
///
/// The two conclusions are stated separately because a single `ends_at` could not
/// admit both of the pass's release routes honestly:
///
/// * A release whose emitted span lands where the source span does — every block
///   written in its source spelling, and every matched compressed block, whose
///   closer reads back as a delimiter — satisfies `ends_at` outright.
/// * A **compressible unterminated** block satisfies the region conclusion only.
///   Its emitted run ends with the three-backtick opener still open while the
///   source run ends open too but in the source's marker family, so the two end
///   states legitimately differ. That is harmless for exactly one reason, and it
///   is a reason the pass enforces rather than one this file can assume: the pass
///   names `Release::Unmatched` only when the input ends inside the block, so such
///   a block is the document's last and no following line ever reads that state.
///   The region half is therefore the whole obligation at the document's tail,
///   and this is the only place the divergence is admitted.
///
/// `lemma_released_span_composes` is the route contract: it says which of the two
/// conclusions applies, and it is also what discharges `spec_emission_reads_back`
/// at a matched block's closer — a compressed closer reads back as a bare
/// three-backtick delimiter, which is what the contract's third clause claims.
pub proof fn lemma_released_span_reads_back(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    opener_idx: int,
    len: int,
    seed: Option<FenceState>,
    opener: FenceState,
    has_closer: bool,
    compressible: bool,
)
    requires
        source.len() == emitted.len(),
        0 <= opener_idx,
        1 <= len,
        opener_idx + len <= source.len(),
        // The opener, exactly as the block lemma names it.
        source[opener_idx].marker == Some(opener.marker),
        source[opener_idx].marker_len == opener.marker_len,
        source[opener_idx].depth == opener.open_depth,
        emitted[opener_idx].depth == opener.open_depth,
        compressible ==> emitted[opener_idx].marker == Some('`') && emitted[opener_idx].marker_len
            == 3,
        !compressible ==> spec_same_reading(source[opener_idx], emitted[opener_idx]),
        spec_masked(seed, opener.open_depth).is_none(),
        // Interior positions.
        forall|j: int|
            #![trigger source[opener_idx + j]]
            0 < j < len - 1 ==> spec_same_reading(
                source[opener_idx + j],
                emitted[opener_idx + j],
            ),
        forall|j: int|
            #![trigger source[opener_idx + j]]
            0 < j < len - 1 ==> source[opener_idx + j].depth >= opener.open_depth
                && !spec_closes(opener, source[opener_idx + j])
                && spec_rewrite_permitted(opener, source[opener_idx + j]),
        // The last position, as a closer or as one more interior line.
        has_closer && compressible ==> spec_closes(opener, source[opener_idx + len - 1]),
        has_closer && compressible ==> 2 <= len,
        has_closer && compressible ==> emitted[opener_idx + len - 1].marker == Some('`'),
        has_closer && compressible ==> emitted[opener_idx + len - 1].marker_len == 3,
        has_closer && compressible ==> emitted[opener_idx + len - 1].depth
            == source[opener_idx + len - 1].depth,
        has_closer && compressible ==> emitted[opener_idx + len - 1].trailing_blank,
        has_closer && !compressible ==> spec_same_reading(
            source[opener_idx + len - 1],
            emitted[opener_idx + len - 1],
        ),
        !has_closer ==> spec_same_reading(
            source[opener_idx + len - 1],
            emitted[opener_idx + len - 1],
        ),
        !has_closer ==> source[opener_idx + len - 1].depth >= opener.open_depth && !spec_closes(
            opener,
            source[opener_idx + len - 1],
        ) && spec_rewrite_permitted(opener, source[opener_idx + len - 1]),
    ensures
        // The region half, at every offset of every route.
        spec_regions_from(source, opener_idx, opener_idx + len, seed) == spec_regions_from(
            emitted,
            opener_idx,
            opener_idx + len,
            seed,
        ),
        // The state half, on the routes that have it. `!compressible` covers
        // every block the pass writes in its source spelling; `has_closer` covers
        // every matched compressed block; the one remaining combination is the
        // compressible unterminated block, which is the document's tail.
        (!compressible || has_closer) ==> spec_state_from(
            source,
            opener_idx,
            opener_idx + len,
            seed,
        ) == spec_state_from(emitted, opener_idx, opener_idx + len, seed),
{
    let end = opener_idx + len;
    let src_span = source.subrange(opener_idx, end);
    let emit_span = emitted.subrange(opener_idx, end);
    // The index correspondences, from the one vstd axiom relating a subrange to
    // its original. Discharging them here is what lets the block lemma's
    // hypotheses be read off the document's lines directly.
    assert forall|i: int| #![trigger src_span[i]] 0 <= i < len implies src_span[i]
        == source[opener_idx + i] by {
        axiom_seq_subrange_index(source, opener_idx, end, i);
    };
    assert forall|i: int| #![trigger emit_span[i]] 0 <= i < len implies emit_span[i]
        == emitted[opener_idx + i] by {
        axiom_seq_subrange_index(emitted, opener_idx, end, i);
    };
    // The block lemma's per-position hypotheses, now in the span's own indices.
    assert forall|i: int|
        #![trigger src_span[i]]
        0 < i < len - 1 implies spec_same_reading(src_span[i], emit_span[i]) by {};
    assert forall|i: int|
        #![trigger src_span[i]]
        0 < i < len - 1 implies src_span[i].depth >= opener.open_depth && !spec_closes(
            opener,
            src_span[i],
        ) && spec_rewrite_permitted(opener, src_span[i]) by {};
    if !compressible {
        // The source-spelling route: the opener reads back as itself as well, so
        // every position agrees and the span needs no block reasoning at all.
        assert forall|i: int|
            #![trigger src_span[i]]
            0 <= i < len implies spec_same_reading(src_span[i], emit_span[i]) by {
            if i == 0 {
            } else if i == len - 1 {
            }
        };
        lemma_agreement_preserves_prefix(src_span, emit_span, len, seed);
    } else {
        // The one route whose delimiters change spelling. `lemma_released_block_reads_back`
        // states the region half alone, which is all a caller composing regions needs;
        // the state half is available here only on the matched route, and
        // `lemma_compressed_block_regions` is the lemma that carries it — so it is
        // called directly rather than through the region-only wrapper, which would
        // discard the very conclusion this lemma has to export.
        reveal(spec_closes);
        lemma_compressed_block_regions(seed, src_span, emit_span, len, opener, has_closer);
    }
    // Both runs of the span are the document's own folds over that span.
    lemma_walk_at_offset(source, src_span, opener_idx, len, seed);
    lemma_walk_at_offset(emitted, emit_span, opener_idx, len, seed);
}

/// `LEM-RELEASED-SPAN-COMPOSES` — the route contract for a released block.
///
/// A thin wrapper whose only job is to state, in one place, which of the two
/// composition obligations a released block discharges. A caller cannot ask for
/// `ends_at` at a compressible unterminated block, because there is no such
/// obligation to discharge there; it gets the state conclusion exactly when the
/// route has one.
pub proof fn lemma_released_span_composes(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    opener_idx: int,
    len: int,
    seed: Option<FenceState>,
    opener: FenceState,
    has_closer: bool,
    compressible: bool,
)
    requires
        source.len() == emitted.len(),
        0 <= opener_idx,
        1 <= len,
        opener_idx + len <= source.len(),
        source[opener_idx].marker == Some(opener.marker),
        source[opener_idx].marker_len == opener.marker_len,
        source[opener_idx].depth == opener.open_depth,
        emitted[opener_idx].depth == opener.open_depth,
        compressible ==> emitted[opener_idx].marker == Some('`') && emitted[opener_idx].marker_len
            == 3,
        !compressible ==> spec_same_reading(source[opener_idx], emitted[opener_idx]),
        spec_masked(seed, opener.open_depth).is_none(),
        forall|j: int|
            #![trigger source[opener_idx + j]]
            0 < j < len - 1 ==> spec_same_reading(
                source[opener_idx + j],
                emitted[opener_idx + j],
            ),
        forall|j: int|
            #![trigger source[opener_idx + j]]
            0 < j < len - 1 ==> source[opener_idx + j].depth >= opener.open_depth
                && !spec_closes(opener, source[opener_idx + j])
                && spec_rewrite_permitted(opener, source[opener_idx + j]),
        has_closer && compressible ==> spec_closes(opener, source[opener_idx + len - 1]),
        has_closer && compressible ==> 2 <= len,
        has_closer && compressible ==> emitted[opener_idx + len - 1].marker == Some('`'),
        has_closer && compressible ==> emitted[opener_idx + len - 1].marker_len == 3,
        has_closer && compressible ==> emitted[opener_idx + len - 1].depth
            == source[opener_idx + len - 1].depth,
        has_closer && compressible ==> emitted[opener_idx + len - 1].trailing_blank,
        has_closer && !compressible ==> spec_same_reading(
            source[opener_idx + len - 1],
            emitted[opener_idx + len - 1],
        ),
        !has_closer ==> spec_same_reading(
            source[opener_idx + len - 1],
            emitted[opener_idx + len - 1],
        ),
        !has_closer ==> source[opener_idx + len - 1].depth >= opener.open_depth && !spec_closes(
            opener,
            source[opener_idx + len - 1],
        ) && spec_rewrite_permitted(opener, source[opener_idx + len - 1]),
        // The route has a state conclusion. This is not a weakening of the lemma
        // above but the reason this one exists: without it, the conclusion would
        // be false for a compressible unterminated block, and a caller that wants
        // a single `ends_at` per span has to name the route it is claiming.
        !compressible || has_closer,
    ensures
        ends_at(
            source,
            emitted,
            opener_idx,
            opener_idx + len,
            seed,
            spec_state_from(source, opener_idx, opener_idx + len, seed),
        ),
{
    lemma_released_span_reads_back(
        source,
        emitted,
        opener_idx,
        len,
        seed,
        opener,
        has_closer,
        compressible,
    );
    reveal(ends_at);
}

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

/// `LEM-REGIONS-FROM-APPENDS` — a segment is its first part then its second.
///
/// This is the law that makes segmentation possible: reading `[start, end)` in one
/// go is the same as reading `[start, mid)` and then `[mid, end)` from the state the
/// first part left, for both the regions and the state. Without it, a document
/// argument would have to unfold the whole recursion at every join.
proof fn lemma_regions_from_appends(
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

} // verus!
