//! The released-block route contract: which composition obligation a release owes.
//!
//! The parent's block lemma is stated about a block's own lines, read from
//! position zero. This file carries it to the document's offsets and states, in
//! one place, which of the two composition conclusions a given release discharges.
//! A caller cannot ask for the state conclusion at a compressible unterminated
//! block, because there is no such obligation to discharge there; separating the
//! routes is what keeps that distinction in the types rather than in the caller's
//! discipline.
//!
//! The hypotheses are the block lemma's, restated at document offsets: the
//! agenda the release wrote, the emission contract instantiated at every
//! position, and the guarded-interior facts compression needs. What this file
//! supplies is the part that is not about the pass at all — that those per-line
//! facts, wherever the block sits, give the composition the two facts it asks
//! for.
//!
//! The child is declared by `verus/plan_spec_compose.rs`; it depends on the
//! facade for the span predicate and the segment specifications, and on its
//! sibling `compose_span` for the copy-walk lemma that translates the block's
//! lines to the position the block occupies.

use crate::fence_spec::{spec_closes, spec_rewrite_permitted};
use crate::production_fence::{FenceState, LineFeatures};
use crate::production_plan::{Agendum, Release};
use vstd::seq::{axiom_seq_subrange_index, axiom_seq_subrange_len};


use super::super::{
    lemma_released_block_reads_back, spec_agendum_line, spec_block_agenda, spec_emission_reads_back,
    spec_masked,
};
use super::{ ends_at, spec_regions_from, spec_state_from };

use super::span::lemma_segment_equals_its_subrange;

use vstd::prelude::*;

verus! {

/// `LEM-RELEASED-SPAN-SATISFIES-ENDS-AT` — a block in the middle of a document.
///
/// This is the join between the two halves of the document theorem. The block
/// lemma is stated over a block's *own* lines, read from position zero, because
/// that is the only way its inductions close. The composition law consumes spans
/// of the whole document, named by their offsets. This lemma translates between
/// those two views: it makes the block's two spans out of the document's lines,
/// carries the agenda and the contract across unchanged, and chains the results.
///
/// The two conclusions are stated separately because a single `ends_at` could not
/// admit both of the pass's release routes honestly:
///
/// * A release whose emitted span lands where the source span does satisfies
///   `ends_at` outright, and every route whose delimiters are not compressed
///   does: an abandoned block writes every line verbatim, and a normalized block
///   respells its delimiters in the source marker family and run. The raised
///   blank tag a dropped `null` specifier produces is admitted here rather than
///   excluded — that is the point of carrying the emission contract instead of
///   plain agreement. A matched compressed block is admitted through the other
///   half of the route.
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
/// conclusions applies, and it names the route condition the caller must supply.
pub proof fn lemma_released_span_reads_back(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    agenda: Seq<Agendum>,
    opener_idx: int,
    len: int,
    seed: Option<FenceState>,
    opener: FenceState,
    release: Release,
    eligible: bool,
    compressible: bool,
)
    requires
        source.len() == emitted.len(),
        0 <= opener_idx,
        1 <= len,
        opener_idx + len <= source.len(),
        agenda == spec_block_agenda(len, release, eligible, compressible),
        // The opener, exactly as the block lemma names it.
        source[opener_idx].marker == Some(opener.marker),
        source[opener_idx].marker_len == opener.marker_len,
        source[opener_idx].depth == opener.open_depth,
        spec_masked(seed, opener.open_depth).is_none(),
        // The emission contract at every position of the span.
        forall|j: int|
            #![trigger source[opener_idx + j]]
            0 <= j < len ==> spec_emission_reads_back(
                agenda[j],
                spec_agendum_line(
                    source[opener_idx + j],
                    #[trigger] emitted[opener_idx + j],
                    agenda[j],
                ),
            ),
        release == Release::Matched ==> spec_closes(opener, source[opener_idx + len - 1]),
        release == Release::Matched ==> 2 <= len,
        // The guarded-interior facts, conditional on the delimiters changing.
        compressible && release == Release::Matched ==> forall|j: int|
            #![trigger source[opener_idx + j]]
            0 < j < len - 1 ==> source[opener_idx + j].depth >= opener.open_depth
                && !spec_closes(opener, source[opener_idx + j])
                && spec_rewrite_permitted(opener, source[opener_idx + j]),
        compressible && release != Release::Matched ==> forall|j: int|
            #![trigger source[opener_idx + j]]
            0 < j < len ==> source[opener_idx + j].depth >= opener.open_depth
                && !spec_closes(opener, source[opener_idx + j])
                && spec_rewrite_permitted(opener, source[opener_idx + j]),
    ensures
        // The region half, at every offset of every route.
        spec_regions_from(source, opener_idx, opener_idx + len, seed) == spec_regions_from(
            emitted,
            opener_idx,
            opener_idx + len,
            seed,
        ),
        // The state half, on the routes that have it.
        (!eligible || !compressible || release == Release::Matched) ==> spec_state_from(
            source,
            opener_idx,
            opener_idx + len,
            seed,
        ) == spec_state_from(emitted, opener_idx, opener_idx + len, seed),
{
    let end = opener_idx + len;
    let src_span = source.subrange(opener_idx, end);
    let emit_span = emitted.subrange(opener_idx, end);
    // The span lengths and the element correspondence, from the two vstd axioms
    // that relate a subrange to its original. Discharging them here is what lets
    // the block lemma's hypotheses be read off the document's lines directly.
    axiom_seq_subrange_len(source, opener_idx, end);
    axiom_seq_subrange_len(emitted, opener_idx, end);
    assert(src_span.len() == len);
    assert(emit_span.len() == len);
    assert forall|i: int| #![trigger src_span[i]] 0 <= i < len implies src_span[i]
        == source[opener_idx + i] by {
        axiom_seq_subrange_index(source, opener_idx, end, i);
    };
    assert forall|i: int| #![trigger emit_span[i]] 0 <= i < len implies emit_span[i]
        == emitted[opener_idx + i] by {
        axiom_seq_subrange_index(emitted, opener_idx, end, i);
    };
    // The contract, in the span's own indices.
    assert forall|i: int| #![trigger src_span[i]]
        0 <= i < len implies spec_emission_reads_back(
            agenda[i],
            spec_agendum_line(src_span[i], emit_span[i], agenda[i]),
        ) by {
        axiom_seq_subrange_index(source, opener_idx, end, i);
        axiom_seq_subrange_index(emitted, opener_idx, end, i);
    };
    // The opener's own facts, at index zero of the span.
    axiom_seq_subrange_index(source, opener_idx, end, 0);
    assert(src_span[0] == source[opener_idx]);
    if release == Release::Matched {
        axiom_seq_subrange_index(source, opener_idx, end, len - 1);
        assert(src_span[len - 1] == source[opener_idx + len - 1]);
        if compressible {
            assert forall|i: int| #![trigger src_span[i]]
                0 < i < len - 1 implies src_span[i].depth >= opener.open_depth && !spec_closes(
                    opener,
                    src_span[i],
                ) && spec_rewrite_permitted(opener, src_span[i]) by {
                axiom_seq_subrange_index(source, opener_idx, end, i);
            };
        }
    } else if compressible {
        assert forall|i: int| #![trigger src_span[i]]
            0 < i < len implies src_span[i].depth >= opener.open_depth && !spec_closes(
                opener,
                src_span[i],
            ) && spec_rewrite_permitted(opener, src_span[i]) by {
            axiom_seq_subrange_index(source, opener_idx, end, i);
        };
    }
    lemma_released_block_reads_back(
        seed,
        src_span,
        emit_span,
        agenda,
        len,
        opener,
        release,
        eligible,
        compressible,
    );
    // Both runs of the span are the document's own folds over that span.
    lemma_segment_equals_its_subrange(source, opener_idx, end, seed);
    lemma_segment_equals_its_subrange(emitted, opener_idx, end, seed);
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
    agenda: Seq<Agendum>,
    opener_idx: int,
    len: int,
    seed: Option<FenceState>,
    opener: FenceState,
    release: Release,
    eligible: bool,
    compressible: bool,
)
    requires
        source.len() == emitted.len(),
        0 <= opener_idx,
        1 <= len,
        opener_idx + len <= source.len(),
        agenda == spec_block_agenda(len, release, eligible, compressible),
        source[opener_idx].marker == Some(opener.marker),
        source[opener_idx].marker_len == opener.marker_len,
        source[opener_idx].depth == opener.open_depth,
        spec_masked(seed, opener.open_depth).is_none(),
        forall|j: int|
            #![trigger source[opener_idx + j]]
            0 <= j < len ==> spec_emission_reads_back(
                agenda[j],
                spec_agendum_line(
                    source[opener_idx + j],
                    #[trigger] emitted[opener_idx + j],
                    agenda[j],
                ),
            ),
        release == Release::Matched ==> spec_closes(opener, source[opener_idx + len - 1]),
        release == Release::Matched ==> 2 <= len,
        compressible && release == Release::Matched ==> forall|j: int|
            #![trigger source[opener_idx + j]]
            0 < j < len - 1 ==> source[opener_idx + j].depth >= opener.open_depth
                && !spec_closes(opener, source[opener_idx + j])
                && spec_rewrite_permitted(opener, source[opener_idx + j]),
        compressible && release != Release::Matched ==> forall|j: int|
            #![trigger source[opener_idx + j]]
            0 < j < len ==> source[opener_idx + j].depth >= opener.open_depth
                && !spec_closes(opener, source[opener_idx + j])
                && spec_rewrite_permitted(opener, source[opener_idx + j]),
        // The route has a state conclusion. Without it, the conclusion would be
        // false for a compressible unterminated block, and a caller that wants a
        // single `ends_at` per span has to name the route it is claiming.
        !eligible || !compressible || release == Release::Matched,
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
        agenda,
        opener_idx,
        len,
        seed,
        opener,
        release,
        eligible,
        compressible,
    );
    reveal(ends_at);
}

} // verus!