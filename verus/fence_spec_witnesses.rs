//! Non-vacuity witnesses for the fence normalization lemma.
//!
//! [`lemma_normalization_preserves_regions`] quantifies over every block whose
//! interior satisfies a guard. A theorem whose hypotheses no block satisfies
//! would be true and worth nothing, so each hypothesis needs a witness, and the
//! guard needs a witness of its own showing it rejects the defect it exists to
//! catch.
//!
//! These live apart from the specification they protect because they answer a
//! different question. The specification says what is true of a guarded block;
//! this file says the question is not empty — one witness satisfies every
//! hypothesis, and a second exhibits a block the guard excludes. Keeping the two
//! apart keeps the specification readable as a statement rather than as a
//! statement plus its evidence, and keeps each file inside the repository's
//! 400-line ceiling (`AGENTS.md`).
//!
//! The witnesses are `proof fn`s with no callers: each is checked because it is
//! compiled, not because something invokes it. The executable half of the same
//! evidence is the sweep in `tests/fence_regions.rs`.

use crate::production_fence::{FenceState, LineFeatures, Region};
use vstd::prelude::*;

use super::{
    lemma_normalization_preserves_regions, spec_compressed, spec_compression_changes_region,
    spec_fence_region, spec_regions_seeded, spec_rewrite_permitted, spec_state_seeded,
};

verus! {

/// The opener used by this file's non-vacuity witnesses.
///
/// Named for the block rather than for the witnesses, because
/// `verus/plan_spec.rs` has a witness opener of its own and the two must not be
/// conflated: this one is a four-tilde opener, and that one is four backticks
/// chosen so the compressed form it produces is a *different* delimiter rather
/// than the same characters again.
pub open spec fn spec_guard_witness_opener() -> FenceState {
    FenceState { marker: '~', marker_len: 4, open_depth: 0 }
}

/// The one-line interior used by the non-vacuity witnesses below.
///
/// A prose line at the opening depth: not fence-shaped, at the opener's own
/// depth, and therefore untouched by the compression predicate. It is built
/// from `Seq::empty` and `push` rather than the `seq!` macro so that its length
/// and contents follow from the sequence axioms directly, without the array
/// view machinery the macro introduces.
pub open spec fn spec_guard_witness_body() -> Seq<LineFeatures> {
    Seq::<LineFeatures>::empty().push(
        LineFeatures { depth: 0, marker: None, marker_len: 0, trailing_blank: false },
    )
}

/// The hypotheses of the normalization lemma are satisfiable.
///
/// Without a witness the theorem could be true of no block at all and the proof
/// would be worth nothing. A prose line at the opening depth satisfies every
/// hypothesis — it carries no marker to close anything — and the lemma then
/// reports that both runs agree on it. The body additionally pins the value the
/// two runs agree on, so the conclusion is a region sequence and not the empty
/// one. The companion executable tests in `tests/fence_regions.rs` exercise the
/// same witness end to end.
proof fn lemma_witness_interior_is_literal()
    ensures
        spec_regions_seeded(spec_guard_witness_body(), 1, Some(spec_guard_witness_opener()))
            == spec_regions_seeded(
                spec_guard_witness_body(),
                1,
                Some(spec_compressed(spec_guard_witness_opener())),
            ),
        spec_regions_seeded(spec_guard_witness_body(), 1, Some(spec_guard_witness_opener()))
            == Seq::<Region>::empty().push(Region::Literal),
{
    let body = spec_guard_witness_body();
    lemma_normalization_preserves_regions(body, 1, spec_guard_witness_opener());
    // Unfold the definitions at the witness: one prose line under an open fence
    // is literal content, and the second postcondition pins that rather than
    // leaving the theorem's conclusion free to be an empty sequence. Recursive
    // `spec fn`s are opaque to the solver until revealed.
    reveal(spec_regions_seeded);
    reveal(spec_state_seeded);
    reveal(spec_fence_region);
    assert(spec_state_seeded(body, 0, Some(spec_guard_witness_opener()))
        == Some(spec_guard_witness_opener()));
    assert(spec_fence_region(Some(spec_guard_witness_opener()), body[0]) == Region::Literal);
    assert(spec_regions_seeded(body, 1, Some(spec_guard_witness_opener()))
        == spec_regions_seeded(body, 0, Some(spec_guard_witness_opener())).push(
            spec_fence_region(
                spec_state_seeded(body, 0, Some(spec_guard_witness_opener())),
                body[0],
            ),
        ));
}

/// The guard is not vacuous: it rejects the defect behind issue #480.
///
/// A three-backtick line inside a four-backtick block is interior content of
/// the opener's own family, so compressing the opener to three backticks would
/// let that line close the block and move every later line out of the literal
/// region. The predicate reports the conflict, which is why the pass preserves
/// the source delimiter, and why the normalization lemma applies to no such
/// block.
proof fn lemma_witness_conflict_is_rejected()
    ensures
        spec_compression_changes_region(
            FenceState { marker: '`', marker_len: 4, open_depth: 0 },
            LineFeatures { depth: 0, marker: Some('`'), marker_len: 3, trailing_blank: true },
        ),
        !spec_rewrite_permitted(
            FenceState { marker: '`', marker_len: 4, open_depth: 0 },
            LineFeatures { depth: 0, marker: Some('`'), marker_len: 3, trailing_blank: true },
        ),
{
}

} // verus!
