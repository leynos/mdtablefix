//! The released-block route contract: which composition obligation a release owes.
//!
//! The parent's block lemmas are stated about a block's own lines, read from
//! position zero. This file carries them to the document's offsets and states, in
//! one place, which of the two composition conclusions a given release discharges.
//! A caller cannot ask for the state conclusion at a compressible unterminated
//! block, because there is no such obligation to discharge there; separating the
//! routes is what keeps that distinction in the types rather than in the caller's
//! discipline.
//!
//! The child is declared by `verus/plan_spec_compose.rs`; it depends on the
//! facade for the span predicate and the segment specifications, and on its
//! sibling `compose_span` for the copy-walk lemma that translates the block's
//! lines to the position the block occupies.

use crate::fence_spec::{spec_closes, spec_rewrite_permitted};
use crate::production_fence::{FenceState, LineFeatures};
use vstd::seq::axiom_seq_subrange_index;


use super::super::{ lemma_agreement_preserves_prefix, lemma_compressed_block_regions, spec_masked, spec_same_reading };
use super::{ ends_at, spec_regions_from, spec_state_from };

use super::span::lemma_walk_at_offset;

use vstd::prelude::*;

verus! {

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

} // verus!
