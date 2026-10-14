//! Block-level non-vacuity witnesses for the planner's lemma library.
//!
//! `verus/plan_spec.rs` states the planner's results; this file supplies the
//! evidence that their hypotheses are satisfiable and their conclusions have
//! content. Extracted so that the specification reads as a statement rather
//! than as a statement plus its evidence, and so that the specification itself
//! comes back under the 400-line ceiling `AGENTS.md` sets.
//!
//! The witnesses here are the *block-level* ones: each reads the single line
//! pair or the single block its lemma is about, so the fixtures at the top are
//! the snippets those lemmas quantify over. The document-level witnesses, which
//! run the whole fold over a source sequence, live in the [`fold`] child module
//! declared at the foot of this file.
//!
//! The split between the two is by what the witness quantifies over rather than
//! by which lemma it protects, because that is what decides the fixtures it
//! needs: the block-level file reuses a handful of small `LineFeatures` values,
//! while the fold witnesses each build their own source sequence and rewrite
//! vector and read back an agenda.

use crate::fence_spec::{
    spec_closes, spec_compressed, spec_fence_next, spec_fence_region, spec_regions_seeded,
    spec_state_seeded,
};
// Imported from the parent rather than through `crate`'s re-export of
// `plan_spec`, matching the sibling `verus/fence_spec_witnesses.rs`. The two
// routes are interchangeable here — both were measured — but naming the
// definition's own module keeps the dependency visible in the source rather
// than routed through `lib.rs`'s glob.
use super::{lemma_compressed_block_regions, spec_delimiters_eligible, spec_fenced, spec_masked};
use crate::production_fence::{FenceState, LineFeatures, Region};
use crate::production_plan::Release;
use vstd::prelude::*;

verus! {

// ---------------------------------------------------------------------------
// Non-vacuity witnesses
// ---------------------------------------------------------------------------

/// A prose line at the opening depth.
///
/// Built from `Seq::empty` and `push` rather than the `seq!` macro so its length
/// and contents follow from the sequence axioms directly, without the array view
/// machinery the macro introduces.
pub open spec fn spec_witness_prose() -> LineFeatures {
    LineFeatures { depth: 0, marker: None, marker_len: 0, trailing_blank: false }
}

/// The four-backtick opener used by the witnesses below.
pub open spec fn spec_witness_opener() -> FenceState {
    FenceState { marker: '`', marker_len: 4, open_depth: 0 }
}

/// A four-backtick delimiter at depth zero.
pub open spec fn spec_witness_bare_backticks(len: usize) -> LineFeatures {
    LineFeatures { depth: 0, marker: Some('`'), marker_len: len, trailing_blank: true }
}

/// The reading agreement lemma is satisfiable and reports a real region.
///
/// Without a witness the theorem could be true of no pair of lines at all and
/// the proof would be worth nothing. Two prose lines agree, and the lemma then
/// reports that both runs classify them alike — which for a seed outside any
/// fence is prose, a region the body pins rather than leaving free.
proof fn lemma_witness_agreement_is_live()
    ensures
        spec_regions_seeded(
            Seq::<LineFeatures>::empty().push(spec_witness_prose()),
            1,
            None,
        ) == spec_regions_seeded(
            Seq::<LineFeatures>::empty().push(spec_witness_prose()),
            1,
            None,
        ),
        spec_fence_region(None, spec_witness_prose()) == Region::Prose,
{
}

/// The compressed-block hypothesis set is satisfiable, and its conclusion is
/// a non-empty region sequence.
///
/// The block is the shortest one that exercises both halves of the argument: a
/// four-backtick opener, one prose line, and a four-backtick closer, compressed
/// to three backticks at each end. Its regions are the same three regions
/// either way, which is the theorem's conclusion made concrete rather than left
/// as an equality between two unknowns.
proof fn lemma_witness_compressed_block_is_live()
    ensures
        spec_regions_seeded(
            Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4)).push(
                spec_witness_prose(),
            ).push(spec_witness_bare_backticks(4)),
            3,
            None,
        ) == Seq::<Region>::empty().push(Region::Delim).push(Region::Literal).push(
            Region::Delim,
        ),
{
    let source = Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4)).push(
        spec_witness_prose(),
    ).push(spec_witness_bare_backticks(4));
    let emitted = Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(3)).push(
        spec_witness_prose(),
    ).push(spec_witness_bare_backticks(3));
    lemma_compressed_block_regions(None, source, emitted, 3, spec_witness_opener(), true);
    // Unfold both recursive specifications at the witness with enough fuel to
    // reach their zero-length base cases, then name the three regions one at a
    // time. Stating the sequence in full is what makes this a non-vacuity
    // witness: the theorem's conclusion is pinned to a value, not left as an
    // equality between two unknowns.
    reveal_with_fuel(spec_state_seeded, 8);
    reveal_with_fuel(spec_regions_seeded, 4);
    reveal(spec_fence_region);
    reveal(spec_fence_next);
    reveal(spec_masked);
    reveal(spec_closes);
    reveal(spec_compressed);
    assert(spec_state_seeded(source, 0, None).is_none());
    assert(spec_fence_region(None, source[0]) == Region::Delim);
    assert(spec_regions_seeded(source, 1, None) == Seq::<Region>::empty().push(Region::Delim));
    assert(spec_fence_region(Some(spec_witness_opener()), source[1]) == Region::Literal);
    assert(spec_regions_seeded(source, 2, None) == Seq::<Region>::empty().push(Region::Delim).push(
        Region::Literal,
    ));
    assert(spec_closes(spec_witness_opener(), source[2]));
    assert(spec_fence_region(Some(spec_witness_opener()), source[2]) == Region::Delim);
    assert(spec_regions_seeded(source, 3, None) == Seq::<Region>::empty().push(Region::Delim).push(
        Region::Literal,
    ).push(Region::Delim));
}

/// `LEM-COMPRESSION-CONFLICT-IS-LOAD-BEARING`.
///
/// The block hypothesis is not vacuous and not a convenience: drop it and the
/// conclusion is false. An interior three-backtick line inside a four-backtick
/// block is literal content under the source opener but a closing delimiter under
/// the compressed one, so the line after it leaves the literal region. This is
/// issue #480 stated as a pair of region values, and it is why the planner keeps
/// a conflict flag at all.
proof fn lemma_witness_conflict_moves_region()
    ensures
        !spec_closes(spec_witness_opener(), spec_witness_bare_backticks(3)),
        spec_fence_region(
            Some(spec_witness_opener()),
            spec_witness_bare_backticks(3),
        ) == Region::Literal,
        spec_fence_region(
            Some(spec_compressed(spec_witness_opener())),
            spec_witness_bare_backticks(3),
        ) == Region::Delim,
        spec_fence_next(
            Some(spec_compressed(spec_witness_opener())),
            spec_witness_bare_backticks(3),
        ).is_none(),
{
}

/// `LEM-WITNESS-MARKER-FAMILY-IS-LOAD-BEARING`.
///
/// A tilde line against a backtick opener is the shape the fence mutation gate
/// drops, and this witness pins what the correct rule says about it: the line
/// does not close the block, so it stays literal, and the opener is still open
/// afterwards. The whole region conclusion would flip if the marker comparison
/// were removed, which is why the mutation is detectable at all.
proof fn lemma_witness_marker_family_holds_the_region()
    ensures
        !spec_closes(spec_witness_opener(), spec_witness_tildes(4)),
        spec_fence_region(Some(spec_witness_opener()), spec_witness_tildes(4)) == Region::Literal,
        spec_fence_next(Some(spec_witness_opener()), spec_witness_tildes(4))
            == Some(spec_witness_opener()),
{
}

/// A four-tilde delimiter at depth zero.
///
/// The marker family the backtick opener above must refuse, so the two
/// witnesses together exercise the closing rule's comparison in the direction
/// the compression pass relies on.
pub open spec fn spec_witness_tildes(len: usize) -> LineFeatures {
    LineFeatures { depth: 0, marker: Some('~'), marker_len: len, trailing_blank: true }
}

/// `LEM-WITNESS-DEPTH-DROP-RELEASES-THE-BLOCK`.
///
/// A line shallower than the opener cannot be content inside it: `CommonMark`
/// ends the block when the blockquote prefix shortens, and the transition
/// checks the depth before it reads the line. The witness pins the three
/// consequences together — the block is gone, the line is prose rather than
/// literal, and it opens a new block of its own depth — which is also why the
/// interior lemma can exclude these lines by hypothesis without losing any
/// line the tracker would actually read as interior.
proof fn lemma_witness_depth_drop_releases_the_block()
    ensures
        spec_fence_next(Some(spec_witness_depth_drop()), spec_witness_shallow_marker())
            == Some(
                FenceState { marker: '`', marker_len: 3, open_depth: 0 },
            ),
        spec_fence_region(Some(spec_witness_depth_drop()), spec_witness_shallow_marker())
            == Region::Delim,
        !spec_fenced(Some(spec_witness_depth_drop()), spec_witness_shallow_marker().depth),
{
}

/// A backtick opener at depth one.
///
/// Paired with the depth-zero marker line below, it exhibits the depth drop that
/// ends the block before the line is read.
pub open spec fn spec_witness_depth_drop() -> FenceState {
    FenceState { marker: '`', marker_len: 3, open_depth: 1 }
}

/// A three-backtick marker at depth zero — the line that drops the blockquote.
pub open spec fn spec_witness_shallow_marker() -> LineFeatures {
    LineFeatures { depth: 0, marker: Some('`'), marker_len: 3, trailing_blank: true }
}

/// `LEM-WITNESS-INCOMPATIBLE-ENDPOINTS-FORBID-REWRITE`.
///
/// An opener that may be respelled together with a closer that may not is the
/// pair `delimiters_eligible` must refuse, and the witness pins the refusal:
/// rewriting only the opener would leave a backtick block facing a tilde closer
/// that no longer closes it. This is the shape that makes the eligibility rule
/// load-bearing rather than a formality, and it is the mixed-family case the
/// fixture corpus does not otherwise reach.
proof fn lemma_witness_incompatible_endpoints_forbid_rewrite()
    ensures
        !spec_delimiters_eligible(Release::Matched, true, false),
        !spec_delimiters_eligible(Release::Matched, false, true),
        spec_delimiters_eligible(Release::Matched, true, true),
        !spec_delimiters_eligible(Release::Abandoned, true, true),
{
}

// ---------------------------------------------------------------------------
// Witnesses over the fold
// ---------------------------------------------------------------------------

/// Non-vacuity witnesses for the document-level fold.
///
/// Declared here rather than in `verus/plan_spec.rs` so that the two witness
/// files stand as siblings and neither is reached through the specification's
/// own module, and so the mutation mirror carries both without a second list.
///
/// The child reuses `spec_witness_prose` and `spec_witness_bare_backticks`
/// above, so it is declared after them.
#[path = "plan_spec_witnesses_fold.rs"]
pub mod fold;

} // verus!
