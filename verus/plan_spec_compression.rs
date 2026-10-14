//! The compressed-block preservation induction.
//!
//! Respelling a block's delimiters to three backticks does change the reading,
//! so unlike the agreement half this argument is a transition argument. The
//! written opener is exactly `spec_compressed` of the source opener, which is
//! the state `verus/fence_spec.rs`'s guarded-interior lemma already compares
//! against; the guard it assumes is the guard the planner's own conflict fold
//! supplies. [`lemma_compressed_block_regions`] is the result the document
//! theorem consumes.
//!
//! Extracted from `verus/plan_spec.rs` at a declaration seam so both files come
//! in under the 400-line ceiling `AGENTS.md` sets. Only the final lemma is
//! `pub`; the three lemmas that build it are private to this module.

use crate::fence_spec::{lemma_block_stays_open, lemma_rewrite_preserves_closer_relation, spec_closes, spec_compressed, spec_fence_next, spec_fence_region, spec_regions_seeded, spec_rewrite_permitted, spec_state_seeded};
use crate::production_fence::{FenceState, LineFeatures, Region};
use super::{lemma_same_reading_preserves_region, spec_masked, spec_same_reading};
use vstd::prelude::*;

verus! {

// ---------------------------------------------------------------------------

/// The three facts a delimiter establishes when the pass starts a block on it.
///
/// A line the tracker read as opening a block carries a marker and arrives with
/// no fence in force, so it classifies as a delimiter and leaves the opener
/// behind. The written form of that line is three backticks at the same depth,
/// so it does the same and leaves `spec_compressed` of the opener behind — the
/// two states `fence_spec.rs`'s guarded-interior lemma compares.
pub(super) proof fn lemma_opener_establishes(
    seed: Option<FenceState>,
    opener: FenceState,
    written: LineFeatures,
)
    requires
        spec_masked(seed, opener.open_depth).is_none(),
        written.marker == Some('`'),
        written.marker_len == 3,
        written.depth == opener.open_depth,
    ensures
        spec_fence_region(seed, written) == Region::Delim,
        spec_fence_next(seed, written) == Some(spec_compressed(opener)),
{
    reveal(spec_fence_region);
    reveal(spec_fence_next);
    reveal(spec_compressed);
    reveal(spec_masked);
    // Unfolding the closing rule is what turns "no fence is in force" into the
    // bare `None` branch of the transition: a masked `Some` is exactly a state
    // the line's depth leaves, and a state the written line closes is likewise
    // not the state it opens against.
    reveal(spec_closes);
}

/// The three facts a delimiter establishes when the pass closes a block on it.
///
/// A line that closes the opener classifies as a delimiter and ends the fence,
/// and so does the three-backtick form written in its place: the run check is a
/// lower bound, so shortening a longer run to three still satisfies it.
pub(super) proof fn lemma_closer_establishes(
    opener: FenceState,
    closer: LineFeatures,
    written: LineFeatures,
)
    requires
        spec_closes(opener, closer),
        written.marker == Some('`'),
        written.marker_len == 3,
        written.depth == closer.depth,
        written.trailing_blank,
        spec_closes(spec_compressed(opener), written),
    ensures
        spec_fence_region(Some(opener), closer) == Region::Delim,
        spec_fence_next(Some(opener), closer).is_none(),
        spec_fence_region(Some(spec_compressed(opener)), written) == Region::Delim,
        spec_fence_next(Some(spec_compressed(opener)), written).is_none(),
{
}

/// `LEM-COMPRESSED-PREFIX` — a compressed block's interior reads identically.
///
/// Every prefix of a compressed block classifies the same way under either
/// opener, and the two runs differ only in the fence state they carry: the
/// source run holds the source opener throughout, and the emitted run holds its
/// three-backtick form. Both are one state, not a growing family, because no
/// interior line closes either spelling.
///
/// The guard is exactly the one `fence_spec.rs` assumes, and
/// [`lemma_no_conflict_all_clear`] is where the planner supplies it. The
/// induction is over the prefix length, so the result holds for blocks of any
/// length; the base case is the opener alone, whose two spellings are the two
/// states the rest of the induction carries.
pub(super) proof fn lemma_compressed_prefix(
    seed: Option<FenceState>,
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    upto: int,
    opener: FenceState,
)
    requires
        1 <= upto,
        upto <= source.len(),
        emitted.len() == source.len(),
        source[0].marker == Some(opener.marker),
        source[0].marker_len == opener.marker_len,
        source[0].depth == opener.open_depth,
        emitted[0].marker == Some('`'),
        emitted[0].marker_len == 3,
        emitted[0].depth == opener.open_depth,
        spec_masked(seed, opener.open_depth).is_none(),
        forall|i: int|
            #![trigger source[i]]
            0 < i < upto ==> spec_same_reading(source[i], #[trigger] emitted[i]),
        forall|i: int|
            #![trigger source[i]]
            0 < i < upto ==> source[i].depth >= opener.open_depth && !spec_closes(
                opener,
                source[i],
            ) && spec_rewrite_permitted(opener, source[i]),
    ensures
        spec_state_seeded(source, upto, seed) == Some(opener),
        spec_state_seeded(emitted, upto, seed) == Some(spec_compressed(opener)),
        spec_regions_seeded(source, upto, seed) == spec_regions_seeded(emitted, upto, seed),
    decreases upto,
{
    reveal(spec_compressed);
    reveal(spec_fence_next);
    reveal(spec_fence_region);
    reveal(spec_masked);
    reveal(spec_closes);
    if upto == 1 {
        lemma_opener_establishes(seed, opener, emitted[0]);
        assert(spec_fence_next(seed, source[0]) == Some(opener));
        assert(spec_fence_region(seed, source[0]) == Region::Delim);
        assert(spec_fence_region(seed, emitted[0]) == Region::Delim);
        // Both one-line runs carry the same single region. The two recursive
        // specifications are unfolded explicitly, with enough fuel for the
        // zero-length case each one recurses into: `reveal` alone supplies one
        // unfolding, and this assertion needs the second.
        assert(spec_state_seeded(source, 0, seed) == seed) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        assert(spec_regions_seeded(source, 0, seed) == Seq::<Region>::empty()) by {
            reveal_with_fuel(spec_regions_seeded, 2);
        };
        assert(spec_regions_seeded(source, 1, seed) == Seq::<Region>::empty().push(Region::Delim))
            by {
            reveal_with_fuel(spec_regions_seeded, 2);
        };
        assert(spec_regions_seeded(emitted, 1, seed) == Seq::<Region>::empty().push(Region::Delim))
            by {
            reveal_with_fuel(spec_regions_seeded, 2);
        };
    } else {
        reveal(spec_state_seeded);
        reveal(spec_regions_seeded);
        lemma_compressed_prefix(seed, source, emitted, upto - 1, opener);
        let line = source[upto - 1];
        assert(spec_same_reading(line, emitted[upto - 1]));
        // The guard, instantiated at the last line: naming the conjuncts here
        // rather than relying on the quantifier's trigger keeps the three
        // lemmas below provable from a hypothesis the solver has actually read.
        assert(line.depth >= opener.open_depth);
        assert(!spec_closes(opener, line));
        assert(spec_rewrite_permitted(opener, line));
        lemma_block_stays_open(opener, line);
        lemma_same_reading_preserves_region(Some(opener), line, emitted[upto - 1]);
        lemma_rewrite_preserves_closer_relation(opener, line);
        assert(spec_state_seeded(source, upto - 1, seed) == Some(opener));
        assert(spec_state_seeded(emitted, upto - 1, seed) == Some(spec_compressed(opener)));
        // The two prefixes agree, and the two readings of this line agree
        // *and* land on the same region, so extending both prefixes by it keeps
        // them equal. Stating the region equation here is deliberate: it gives
        // the verifier the exact term the conclusion needs.
        assert(spec_fence_region(Some(opener), line) == spec_fence_region(
            Some(spec_compressed(opener)),
            emitted[upto - 1],
        ));
        assert(spec_regions_seeded(source, upto, seed) == spec_regions_seeded(
            emitted,
            upto,
            seed,
        ));
    }
}

/// `LEM-COMPRESSED-BLOCK-REGIONS` — a whole compressed block reads identically.
///
/// This is the block half of the document theorem for the one case that changes
/// a delimiter's spelling. It extends [`lemma_compressed_prefix`] over the
/// closing delimiter when the block has one, and reports that the two runs agree
/// on the state afterwards as well — which is what lets one block's result be
/// the next block's precondition.
///
/// The state conclusion is stated only for a matched block. An unterminated
/// block's two runs end in different states — the source opener and its
/// three-backtick form — and that is harmless precisely because an unterminated
/// block is the last one in its document: the pass names `Unmatched` only when
/// the input ends inside the block, so there is no following line for the
/// divergent state to reach.
///
/// Public because `verus/plan_spec_witnesses.rs` applies it to a concrete
/// three-line block, which is how the witness that its hypotheses are
/// satisfiable reuses this result rather than restating it.
pub proof fn lemma_compressed_block_regions(
    seed: Option<FenceState>,
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    len: int,
    opener: FenceState,
    has_closer: bool,
)
    requires
        1 <= len,
        source.len() == len,
        emitted.len() == len,
        source[0].marker == Some(opener.marker),
        source[0].marker_len == opener.marker_len,
        source[0].depth == opener.open_depth,
        emitted[0].marker == Some('`'),
        emitted[0].marker_len == 3,
        emitted[0].depth == opener.open_depth,
        spec_masked(seed, opener.open_depth).is_none(),
        forall|i: int|
            #![trigger source[i]]
            0 < i < len - 1 ==> spec_same_reading(source[i], #[trigger] emitted[i]),
        forall|i: int|
            #![trigger source[i]]
            0 < i < len - 1 ==> source[i].depth >= opener.open_depth && !spec_closes(
                opener,
                source[i],
            ) && spec_rewrite_permitted(opener, source[i]),
        has_closer ==> spec_closes(opener, source[len - 1]),
        // A matched block is at least two lines: the line that opens a block is
        // not the line that closes it, which the pass's own routing enforces by
        // opening on a `Restart` and closing on a later line's `Close`.
        has_closer ==> 2 <= len,
        // What the emitter wrote for the closer: exactly three backticks at the
        // source line's own depth. The closing relation to the compressed opener
        // follows from those facts rather than being assumed.
        has_closer ==> emitted[len - 1].marker == Some('`'),
        has_closer ==> emitted[len - 1].marker_len == 3,
        has_closer ==> emitted[len - 1].depth == source[len - 1].depth,
        has_closer ==> emitted[len - 1].trailing_blank,
        // An unterminated block's last line is interior content too, so it needs
        // the same two guarantees its earlier lines carry.
        !has_closer ==> spec_same_reading(source[len - 1], emitted[len - 1]),
        !has_closer ==> source[len - 1].depth >= opener.open_depth && !spec_closes(
            opener,
            source[len - 1],
        ) && spec_rewrite_permitted(opener, source[len - 1]),
    ensures
        spec_regions_seeded(source, len, seed) == spec_regions_seeded(emitted, len, seed),
        has_closer ==> spec_state_seeded(source, len, seed) == spec_state_seeded(
            emitted,
            len,
            seed,
        ),
    decreases len,
{
    reveal(spec_compressed);
    reveal(spec_closes);
    reveal(spec_state_seeded);
    reveal(spec_regions_seeded);
    if !has_closer {
        // The interior hypotheses name the strict interior; an unterminated
        // block's last line is interior as well, so the wide form the prefix
        // lemma wants is recovered by splitting on whether the index is that
        // last line.
        assert(forall|i: int|
            #![trigger source[i]]
            0 < i < len ==> spec_same_reading(source[i], #[trigger] emitted[i])) by {
            assert forall|i: int|
                #![trigger source[i]]
                0 < i < len implies spec_same_reading(source[i], #[trigger] emitted[i]) by {
                if i == len - 1 {
                }
            }
        };
        assert(forall|i: int|
            #![trigger source[i]]
            0 < i < len ==> source[i].depth >= opener.open_depth && !spec_closes(
                opener,
                source[i],
            ) && spec_rewrite_permitted(opener, source[i])) by {
            assert forall|i: int|
                #![trigger source[i]]
                0 < i < len implies source[i].depth >= opener.open_depth && !spec_closes(
                    opener,
                    source[i],
                ) && spec_rewrite_permitted(opener, source[i]) by {
                if i == len - 1 {
                }
            }
        };
        lemma_compressed_prefix(seed, source, emitted, len, opener);
    } else {
        lemma_compressed_prefix(seed, source, emitted, len - 1, opener);
        let closer = source[len - 1];
        let written = emitted[len - 1];
        assert(spec_state_seeded(source, len - 1, seed) == Some(opener));
        assert(spec_state_seeded(emitted, len - 1, seed) == Some(spec_compressed(opener)));
        // The closer clause of `spec_closes` is a lower bound on the run, so the
        // three-backtick form the emitter writes satisfies it whenever the
        // source form did; `reveal` is what lets the solver see that.
        assert(spec_closes(spec_compressed(opener), written));
        lemma_closer_establishes(opener, closer, written);
        // The final region is a delimiter in both runs, so extending two equal
        // prefixes by it keeps them equal; the final states are both `None`.
        assert(spec_regions_seeded(source, len, seed) == spec_regions_seeded(source, len - 1, seed).push(
            Region::Delim,
        ));
        assert(spec_regions_seeded(emitted, len, seed) == spec_regions_seeded(
            emitted,
            len - 1,
            seed,
        ).push(Region::Delim));
    }
}

} // verus!
