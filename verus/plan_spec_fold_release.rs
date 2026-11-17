//! One released chunk of the fold, read as a preserved span.
//!
//! The composition law consumes spans at document offsets; the block theorem is
//! stated over a block's own lines. `plan_spec_compose_release.rs` is the bridge
//! between them, and this module is the caller side of that bridge: it takes the
//! facts the fold invariant has established about a buffered block and hands the
//! bridge exactly the hypotheses it asks for.
//!
//! The seam is worth one file because the hypotheses are conditional in a way the
//! bridge cannot be. The guarded-interior facts are needed only when the block's
//! delimiters are respelled, and whether they are respelled is the block's own
//! conflict flag. Stating that condition here, once, is what keeps the induction
//! from re-deriving it in each of the three routes that release a block.

use crate::fence_spec::{
    spec_closes, spec_rewrite_permitted, spec_state_seeded,
};
use crate::production_fence::LineFeatures;
use crate::production_plan::{Agendum, Block as PlanBlock, Release};

use super::{
    ends_at, lemma_prefix_splits_at_seeded_state, lemma_released_span_reads_back,
    lemma_segment_is_ends_at, spec_agendum_line, spec_block_agenda, spec_delimiters_eligible,
    spec_emission_reads_back, spec_masked, spec_regions_from, spec_released_agenda,
};

use vstd::prelude::*;

verus! {

/// A chunk the fold released is one preserved span at the document's offsets.
///
/// The hypotheses are the fold invariant's, restated over the span: the opener's
/// three features, the masked guard, the interior lines the block absorbed, and —
/// for a matched release — the closer relation and the length bound. The agenda is
/// named as the release's own spelling rather than as an arbitrary sequence, so a
/// caller cannot claim a spelling the pass would not write.
///
/// The conclusion is the composition's per-span obligation, plus the state
/// agreement the next span needs on every route that has it. An abandoned block
/// always has it — its delimiters are never respelled — which is why the
/// disjunction below is not a burden on the caller.
pub proof fn lemma_block_chunk_composes(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    chunk: Seq<Agendum>,
    start: int,
    end: int,
    block: PlanBlock,
    release: Release,
    opener_rewritable: bool,
    closer_rewritable: bool,
)
    requires
        emitted.len() == source.len(),
        0 <= start < end <= source.len(),
        // The chunk is exactly what the release writes, and the contract holds at
        // every position of it.
        chunk == spec_released_agenda(
            block,
            end - start,
            opener_rewritable,
            closer_rewritable,
            release,
        ),
        forall|j: int|
            #![trigger source[start + j]]
            0 <= j < end - start ==> spec_emission_reads_back(
                chunk[j],
                spec_agendum_line(source[start + j], #[trigger] emitted[start + j], chunk[j]),
            ),
        // The opener, as the block recorded it.
        source[start].marker == Some(block.opening.marker),
        source[start].marker_len == block.opening.marker_len,
        source[start].depth == block.opening.open_depth,
        spec_masked(spec_state_seeded(source, start, None), block.opening.open_depth).is_none(),
        // The two runs already agree at the span's start, which is where the fold
        // has written up to while a block is buffered.
        spec_state_seeded(source, start, None) == spec_state_seeded(emitted, start, None),
        // The interior, as the conflict fold established it. Conditional on the
        // block having no conflict for the same reason the released-span theorem
        // is: a conflict-bearing block is never respelled, so it owes nothing here.
        // The two ranges are the theorem's own — a matched block's closer is a
        // delimiter, not interior content, so it is not asked to be one.
        (!block.conflicts && release == Release::Matched) ==> forall|j: int|
            #![trigger source[j]]
            start < j < end - 1 ==> source[j].depth >= block.opening.open_depth
                && !spec_closes(block.opening, source[j])
                && spec_rewrite_permitted(block.opening, source[j]),
        (!block.conflicts && release != Release::Matched) ==> forall|j: int|
            #![trigger source[j]]
            start < j < end ==> source[j].depth >= block.opening.open_depth
                && !spec_closes(block.opening, source[j])
                && spec_rewrite_permitted(block.opening, source[j]),
        release == Release::Matched ==> spec_closes(block.opening, source[end - 1]),
        release == Release::Matched ==> 2 <= end - start,
    ensures
        // The region half holds on every route, including the compressible
        // unterminated block whose end states legitimately differ.
        spec_regions_from(source, start, end, spec_state_seeded(source, start, None))
            == spec_regions_from(emitted, start, end, spec_state_seeded(source, start, None)),
        // The span predicate is the region half *and* the state half, so it is
        // owed only on the routes that have both. A compressible unterminated
        // block ends with a respelled opener still open, so its emitted run ends
        // in a different state and `ends_at` would be false of it.
        (!spec_delimiters_eligible(release, opener_rewritable, closer_rewritable)
            || block.conflicts || release == Release::Matched) ==> ends_at(
            source,
            emitted,
            start,
            end,
            spec_state_seeded(source, start, None),
            spec_state_seeded(source, end, None),
        ),
        (!spec_delimiters_eligible(release, opener_rewritable, closer_rewritable)
            || block.conflicts || release == Release::Matched) ==> spec_state_seeded(
            source,
            end,
            None,
        ) == spec_state_seeded(emitted, end, None),
{
    let len = end - start;
    let eligible = spec_delimiters_eligible(release, opener_rewritable, closer_rewritable);
    let compressible = !block.conflicts;
    reveal(spec_released_agenda);
    assert(chunk == spec_block_agenda(len, release, eligible, compressible));
    assert(1 <= len);
    assert(compressible && release == Release::Matched ==> forall|j: int|
        #![trigger source[start + j]]
        0 < j < len - 1 ==> source[start + j].depth >= block.opening.open_depth && !spec_closes(
            block.opening,
            source[start + j],
        ) && spec_rewrite_permitted(block.opening, source[start + j]));
    assert(compressible && release != Release::Matched ==> forall|j: int|
        #![trigger source[start + j]]
        0 < j < len ==> source[start + j].depth >= block.opening.open_depth && !spec_closes(
            block.opening,
            source[start + j],
        ) && spec_rewrite_permitted(block.opening, source[start + j]));
    let seed = spec_state_seeded(source, start, None);
    lemma_released_span_reads_back(
        source,
        emitted,
        chunk,
        start,
        len,
        seed,
        block.opening,
        release,
        eligible,
        compressible,
    );
    // The bridge's conclusion names the segment form; the fold names the seeded
    // state at the span's end, and the split law is what makes them one state.
    lemma_prefix_splits_at_seeded_state(source, start, end, None);
    lemma_prefix_splits_at_seeded_state(emitted, start, end, None);
    if !eligible || block.conflicts || release == Release::Matched {
        // The bridge states the state half only on the routes that have one; on
        // those routes the segment equalities it proved are the span predicate.
        assert(!eligible || !compressible || release == Release::Matched);
        lemma_segment_is_ends_at(source, emitted, start, end, seed);
    }
}

} // verus!