//! Specification of the fence transition kernel and its normalization lemma.
//!
//! The functions here are the specification `src/wrap/fence/kernel.rs` must
//! realise. They say nothing about regex, blockquote parsing, or byte offsets:
//! a line arrives already reduced to its [`LineFeatures`], which keeps the
//! proof small enough to close and keeps all recognition outside the trusted
//! boundary, exactly as the classification kernel does.
//!
//! The central result is `LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS`. Its
//! subject is a block's *interior*: the lines between an opening delimiter and
//! the line that closes it. Respelling the opening delimiter may change the
//! block's spelling, and may respell its closing delimiter too, but it may not
//! move an interior line out of the region it already occupied.
//!
//! Regions are specified as a *seeded prefix*: [`spec_regions_seeded`] reads
//! the first `upto` lines from a given fence state, extending its result one
//! region at a time. That is the executable loop's own recurrence, so the loop
//! invariant is the definition rather than an equation between a growing prefix
//! and a shrinking suffix, and no concatenation lemma is needed to check it.

use crate::production_fence::{FenceState, LineFeatures, Region};
use vstd::prelude::*;

verus! {

/// The specification of one fence transition.
///
/// Precedence matches the executable kernel exactly: a depth decrease ends the
/// fence before the line is read, a matching bare marker closes, a non-fence
/// line is literal content or prose, any other marker under an open fence is
/// interior literal content, and a marker with no fence open starts one.
pub open spec fn spec_fence_next(
    state: Option<FenceState>,
    line: LineFeatures,
) -> Option<FenceState> {
    let opened = match state {
        Some(open) if line.depth < open.open_depth => None,
        other => other,
    };
    match opened {
        Some(open) if spec_closes(open, line) => None,
        Some(_) => opened,
        None => match line.marker {
            Some(marker) => Some(FenceState {
                marker,
                marker_len: line.marker_len,
                open_depth: line.depth,
            }),
            None => None,
        },
    }
}

/// The specification of the region one line belongs to.
///
/// Every branch returns [`Region::Literal`] except closing a fence, opening
/// one, and prose under no fence. A fence-shaped line that does none of those
/// is interior content, which is the case issue #480 got wrong.
pub open spec fn spec_fence_region(
    state: Option<FenceState>,
    line: LineFeatures,
) -> Region {
    let opened = match state {
        Some(open) if line.depth < open.open_depth => None,
        other => other,
    };
    match opened {
        Some(open) if spec_closes(open, line) => Region::Delim,
        // Any fence-shaped line that is not a closer is interior content, and
        // so is every line that is not fence-shaped at all.
        Some(_) => Region::Literal,
        None => match line.marker {
            Some(_) => Region::Delim,
            None => Region::Prose,
        },
    }
}

/// The fence state after reading the first `upto` lines from `seed`.
///
/// The executable loop carries this value as its running state, so the loop's
/// state invariant is this function applied to its own counter.
pub open spec fn spec_state_seeded(
    features: Seq<LineFeatures>,
    upto: int,
    seed: Option<FenceState>,
) -> Option<FenceState>
    decreases upto
{
    if upto <= 0 {
        seed
    } else {
        spec_fence_next(
            spec_state_seeded(features, upto - 1, seed),
            features[upto - 1],
        )
    }
}

/// The regions of the first `upto` lines, read from `seed`.
///
/// The recursion extends its result by one region per line, which is exactly
/// what the executable loop's `out.push` does at the same counter value. That
/// correspondence is what keeps the loop invariant checkable without appealing
/// to associativity of sequence concatenation.
pub open spec fn spec_regions_seeded(
    features: Seq<LineFeatures>,
    upto: int,
    seed: Option<FenceState>,
) -> Seq<Region>
    decreases upto
{
    if upto <= 0 {
        Seq::empty()
    } else {
        spec_regions_seeded(features, upto - 1, seed).push(
            spec_fence_region(spec_state_seeded(features, upto - 1, seed), features[upto - 1]),
        )
    }
}

/// The regions of a whole document, read from its first line.
pub open spec fn spec_regions(features: Seq<LineFeatures>) -> Seq<Region> {
    spec_regions_seeded(features, features.len() as int, None)
}

/// Whether the line's marker closes the fence described by `state`.
///
/// The marker-character check is load-bearing: without it a tilde line would
/// appear to close a backtick opener. That check is what makes the compression
/// predicate discriminating and what the mutation gate removes.
pub open spec fn spec_closes(state: FenceState, line: LineFeatures) -> bool {
    match line.marker {
        Some(marker) => {
            line.depth == state.open_depth
                && marker == state.marker
                && line.marker_len >= state.marker_len
                && line.trailing_blank
        },
        None => false,
    }
}

/// Whether the line would close `state` were it not carrying an info string.
///
/// Mirrors the production `agrees_with_opener`, which lets the tracker tell "a
/// matching delimiter carrying trailing text" from "a delimiter of an
/// unrelated family". Those are different reasons a marker leaves the state
/// unchanged, and the distinction is what the tracing events report.
pub open spec fn spec_agrees_with_opener(
    state: FenceState,
    line: LineFeatures,
) -> bool {
    spec_closes(state, LineFeatures { trailing_blank: true, ..line })
}

/// The delimiter normalization writes in place of a state's opening marker.
pub open spec fn spec_compressed(state: FenceState) -> FenceState {
    FenceState { marker: '`', marker_len: 3, open_depth: state.open_depth }
}

/// Whether `line` is fence-shaped interior content of the block `state` opened.
pub open spec fn spec_interior_delimiter(
    state: FenceState,
    line: LineFeatures,
) -> bool {
    line.marker.is_some() && line.depth >= state.open_depth && !spec_closes(state, line)
}

/// Whether compressing `state`'s delimiter could change `line`'s region.
///
/// This mirrors the executable predicate in `src/wrap/fence/kernel.rs`, and it
/// is the guard every lemma below assumes.
pub open spec fn spec_compression_changes_region(
    state: FenceState,
    line: LineFeatures,
) -> bool {
    spec_interior_delimiter(state, line) && matches!(
        line.marker,
        Some(marker) if marker == state.marker || marker == spec_compressed(state).marker,
    )
}

/// Whether a delimiter rewrite may be performed while this one line is inside
/// the block.
///
/// Rewriting is permitted for every line except interior content whose marker
/// conflicts with the compression target: a fence-shaped line still held open
/// that the rewritten three-backtick opener would newly close. Delimiters and
/// prose are always permitted; only such an interior line forbids the rewrite,
/// because it alone would move later lines out of the literal region.
///
/// This is the whole safety condition, stated as an equivalence so that the
/// negation the lemmas consume is available without a case analysis of its own.
pub open spec fn spec_rewrite_permitted(state: FenceState, line: LineFeatures) -> bool {
    !spec_compression_changes_region(state, line)
}

/// `LEM-REWRITE-PRESERVES-CLOSER-RELATION`.
///
/// A line that neither closes the original opener nor is interior content of
/// its own marker family closes the rewritten opener exactly as it closed the
/// original: that is, not at all.
///
/// The statement is deliberately one-sided. It is *not* true that every
/// non-closer keeps its closer status: a tilde line closes a tilde opener but
/// cannot close the three-backtick opener the pass writes in its place. What
/// makes the normalization safe is that such a line is a delimiter and the pass
/// respells it, so its region is a delimiter's region on both sides. The
/// lemma therefore covers precisely the lines the pass leaves alone.
///
/// The marker-character check is what makes this true: without it a tilde line
/// would appear to close a backtick opener, and the mutation gate is the
/// evidence.
proof fn lemma_rewrite_preserves_closer_relation(
    state: FenceState,
    line: LineFeatures,
)
    requires
        line.depth >= state.open_depth,
        !spec_closes(state, line),
        spec_rewrite_permitted(state, line),
    ensures
        !spec_closes(spec_compressed(state), line),
        spec_fence_region(Some(state), line) == Region::Literal,
        spec_fence_region(Some(spec_compressed(state)), line) == Region::Literal,
{
    if line.marker is Some {
        let marker = line.marker.unwrap();
        // Interior content, so the guard must have rejected the family, and a
        // marker of an unrelated family closes the rewritten opener nowhere.
        assert(marker != state.marker && marker != spec_compressed(state).marker);
    }
}

/// A block stays open across a line that closes neither delimiter.
///
/// Both the original and the rewritten opener survive the line unchanged, so
/// the relation the induction needs is re-established by the step itself.
proof fn lemma_block_stays_open(
    state: FenceState,
    line: LineFeatures,
)
    requires
        line.depth >= state.open_depth,
        !spec_closes(state, line),
        spec_rewrite_permitted(state, line),
    ensures
        spec_fence_next(Some(state), line) == Some(state),
        spec_fence_next(Some(spec_compressed(state)), line)
            == Some(spec_compressed(state)),
{
    lemma_rewrite_preserves_closer_relation(state, line);
}

/// A block stays open across every line of its interior.
///
/// Reading the body from the original opener, or from the rewritten one, leaves
/// that same opener in place at every prefix: no line ends the fence under
/// either spelling. This is what lets the normalization lemma compare the two
/// runs line by line instead of reasoning about the states separately.
///
/// The two conclusions are proved together, because the induction for each one
/// needs the other's hypotheses at the shorter prefix, and because a single
/// traversal documents that the two runs never diverge.
proof fn lemma_state_preserved(
    body: Seq<LineFeatures>,
    upto: int,
    state: FenceState,
)
    requires
        0 <= upto <= body.len(),
        forall|i: int|
            #![trigger body[i]]
            0 <= i < upto ==> body[i].depth >= state.open_depth && !spec_closes(
                state,
                body[i],
            ) && spec_rewrite_permitted(state, body[i]),
    ensures
        spec_state_seeded(body, upto, Some(state)) == Some(state),
        spec_state_seeded(body, upto, Some(spec_compressed(state)))
            == Some(spec_compressed(state)),
    decreases upto,
{
    if upto > 0 {
        lemma_state_preserved(body, upto - 1, state);
        lemma_block_stays_open(state, body[upto - 1]);
    }
}

/// `LEM-FENCE-NORMALIZATION-PRESERVES-REGIONS`, for one block's interior.
///
/// A block every one of whose interior lines is untouched by the compression
/// predicate has the same interior regions under the rewritten opener as under
/// the original. Rewriting the delimiter changes the block's spelling and
/// nothing else.
///
/// The hypotheses are exactly what the pass guarantees while buffering a block:
/// every line still inside it sits at or below the opening depth, because a
/// shallower line ends the fence; no line closes the opener, because the first
/// closer ends the block and is itself a delimiter; and no line is a
/// compression conflict, because a single conflict makes the pass preserve the
/// source delimiter for the whole block.
///
/// The induction is over the prefix length, so the result holds for blocks of
/// any length and for every prefix of one. The guard is what makes it true
/// rather than a convenience: without it a rewritten opener can be closed by an
/// interior line that did not close the original, every later line leaves the
/// literal region, and the conclusion fails. That is issue #480, and the
/// executable sweep in `tests/fence_regions.rs` seeds exactly that fault.
proof fn lemma_normalization_preserves_regions(
    body: Seq<LineFeatures>,
    upto: int,
    state: FenceState,
)
    requires
        0 <= upto <= body.len(),
        forall|i: int|
            #![trigger body[i]]
            0 <= i < upto ==> body[i].depth >= state.open_depth && !spec_closes(
                state,
                body[i],
            ) && spec_rewrite_permitted(state, body[i]),
    ensures spec_regions_seeded(body, upto, Some(state)) == spec_regions_seeded(
        body,
        upto,
        Some(spec_compressed(state)),
    ),
    decreases upto,
{
    if upto > 0 {
        let line = body[upto - 1];
        lemma_state_preserved(body, upto - 1, state);
        lemma_rewrite_preserves_closer_relation(state, line);
        lemma_normalization_preserves_regions(body, upto - 1, state);
    }
}

/// The opener used by the non-vacuity witnesses below.
pub open spec fn spec_witness_opener() -> FenceState {
    FenceState { marker: '~', marker_len: 4, open_depth: 0 }
}

/// The one-line interior used by the non-vacuity witnesses below.
///
/// A prose line at the opening depth: not fence-shaped, at the opener's own
/// depth, and therefore untouched by the compression predicate. It is built
/// from `Seq::empty` and `push` rather than the `seq!` macro so that its length
/// and contents follow from the sequence axioms directly, without the array
/// view machinery the macro introduces.
pub open spec fn spec_witness_body() -> Seq<LineFeatures> {
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
        spec_regions_seeded(spec_witness_body(), 1, Some(spec_witness_opener()))
            == spec_regions_seeded(
                spec_witness_body(),
                1,
                Some(spec_compressed(spec_witness_opener())),
            ),
        spec_regions_seeded(spec_witness_body(), 1, Some(spec_witness_opener()))
            == Seq::<Region>::empty().push(Region::Literal),
{
    let body = spec_witness_body();
    lemma_normalization_preserves_regions(body, 1, spec_witness_opener());
    // Unfold the definitions at the witness: one prose line under an open fence
    // is literal content, and the second postcondition pins that rather than
    // leaving the theorem's conclusion free to be an empty sequence. Recursive
    // `spec fn`s are opaque to the solver until revealed.
    reveal(spec_regions_seeded);
    reveal(spec_state_seeded);
    reveal(spec_fence_region);
    assert(spec_state_seeded(body, 0, Some(spec_witness_opener()))
        == Some(spec_witness_opener()));
    assert(spec_fence_region(Some(spec_witness_opener()), body[0]) == Region::Literal);
    assert(spec_regions_seeded(body, 1, Some(spec_witness_opener()))
        == spec_regions_seeded(body, 0, Some(spec_witness_opener())).push(
            spec_fence_region(
                spec_state_seeded(body, 0, Some(spec_witness_opener())),
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
