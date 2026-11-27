//! The region half of the document theorem.
//!
//! [`lemma_released_block_reads_back`] is the per-block step the composition
//! consumes: every line a released block covers keeps the region it had. It is
//! the meeting point of both halves above, calling the agreement argument for
//! the lines that do not move and the compressed-block induction for the
//! delimiter that does.
//!
//! The lemma's inputs are the agenda the release wrote and the emission contract
//! instantiated at every position of the block. It does not take the emitted
//! readings as free hypotheses: it derives each position's emitter fact from the
//! agendum the driver chose, via [`spec_agendum_line`] and the consumers in the
//! `emission` module. That is what gives the contract a real consumer — weakening
//! one of its arms makes this lemma unprovable rather than leaving the tree
//! clean.
//!
//! Extracted from `verus/plan_spec.rs` at a declaration seam so both files come
//! in under the 400-line ceiling `AGENTS.md` sets. The lemma was `pub(super)`
//! before the move and stays so, which is why the parent re-imports it rather
//! than re-exporting it.

use crate::fence_spec::{
    spec_closes, spec_compressed, spec_regions_seeded, spec_rewrite_permitted, spec_state_seeded,
};
use crate::production_fence::{FenceState, LineFeatures};
use crate::production_plan::{Agendum, Release};
use super::{
    lemma_agreement_preserves_prefix, lemma_closer_reads_back_as_itself,
    lemma_compressed_block_regions, lemma_compressed_emission_shape, lemma_delimited_prefix_agreement,
    lemma_normalized_emission_reads_back, lemma_verbatim_emission_agrees, spec_agendum_line,
    spec_block_agenda, spec_delimiter_agendum, spec_delimiter_reads_back, spec_emission_reads_back,
    spec_is_outer_delimiter, spec_masked, spec_same_reading,
};
use vstd::prelude::*;

verus! {

// ---------------------------------------------------------------------------

/// Whether a released block's region sequence is the same after emission.
///
/// This is the document theorem stated for one block, and it is the form the
/// correctness argument actually needs: a document is a sequence of blocks and
/// the lines between them, a line between blocks is written verbatim and so reads
/// back as itself, and a block is the only place an emission can change a reading.
///
/// The hypothesis the pass cannot prove about itself is supplied as
/// [`spec_emission_reads_back`] over [`spec_agendum_line`] at every position:
/// the pass renders an agendum into bytes and a regular expression reads those
/// bytes back, and Verus cannot reason about a compiled regular expression. The
/// lemma then *derives* the three emission shapes from it, so the contract is
/// consumed rather than restated. Round two on a compressed line is the case
/// worth naming: the bytes differ from the source line's, and the theorem is that
/// the *region* does not.
///
/// The conclusion names the whole block because the block is where the two
/// induction principles above live: interior lines are covered by agreement, and
/// the delimiters by [`lemma_compressed_block_regions`] or
/// [`lemma_delimited_prefix_agreement`]. Nothing here restates either.
pub proof fn lemma_released_block_reads_back(
    seed: Option<FenceState>,
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    agenda: Seq<Agendum>,
    len: int,
    opener: FenceState,
    release: Release,
    eligible: bool,
    compressible: bool,
)
    requires
        1 <= len,
        source.len() == len,
        emitted.len() == len,
        // The agenda is the one the release writes, so every position's spelling
        // is the driver's decision rather than a caller's claim.
        agenda == spec_block_agenda(len, release, eligible, compressible),
        // The opener the kernel opened.
        source[0].marker == Some(opener.marker),
        source[0].marker_len == opener.marker_len,
        source[0].depth == opener.open_depth,
        spec_masked(seed, opener.open_depth).is_none(),
        // The external contract: what the written bytes read back as.
        forall|i: int|
            #![trigger source[i]]
            0 <= i < len ==> spec_emission_reads_back(
                agenda[i],
                spec_agendum_line(source[i], #[trigger] emitted[i], agenda[i]),
            ),
        // A matched release means the last line closed the block under the source
        // reading. The fold reads this off the transition, and it is what turns
        // the closer's read-back relation into plain agreement.
        release == Release::Matched ==> spec_closes(opener, source[len - 1]),
        release == Release::Matched ==> 2 <= len,
        // The guarded-interior facts compression relies on. A block whose
        // delimiters are not rewritten does not need them, which is why they are
        // conditional on `compressible`; requiring them unconditionally would make
        // the conflict-fallback route's hypotheses unsatisfiable, which is exactly
        // the defect this conditional removes.
        compressible && release == Release::Matched ==> forall|i: int|
            #![trigger source[i]]
            0 < i < len - 1 ==> source[i].depth >= opener.open_depth && !spec_closes(
                opener,
                source[i],
            ) && spec_rewrite_permitted(opener, source[i]),
        compressible && release != Release::Matched ==> forall|i: int|
            #![trigger source[i]]
            0 < i < len ==> source[i].depth >= opener.open_depth && !spec_closes(
                opener,
                source[i],
            ) && spec_rewrite_permitted(opener, source[i]),
    ensures
        spec_regions_seeded(source, len, seed) == spec_regions_seeded(emitted, len, seed),
        // A completed release leaves the same fence state behind, which is what
        // lets one span's result be the next span's precondition. The one
        // exception is a compressed block with no closer, which is the document's
        // tail and has no following line for a divergent state to reach.
        (!eligible || !compressible || release == Release::Matched)
            ==> spec_state_seeded(source, len, seed) == spec_state_seeded(emitted, len, seed),
{
    super::planner::lemma_block_agenda_length(len, release, eligible, compressible, len);
    assert(agenda == spec_block_agenda(len, release, eligible, compressible));
    assert(agenda.len() == len);
    // No delimiter is respelled: every position is written byte for byte, so
    // agreement carries the whole block and the state as well.
    if !eligible {
        assert(forall|i: int|
            #![trigger source[i]]
            0 <= i < len ==> spec_same_reading(source[i], #[trigger] emitted[i])) by {
            assert forall|i: int|
                #![trigger source[i]]
                0 <= i < len implies spec_same_reading(source[i], #[trigger] emitted[i]) by {
                super::planner::lemma_block_agenda_index(len, release, false, compressible, i);
                reveal(spec_is_outer_delimiter);
                reveal(spec_delimiter_agendum);
                assert(agenda[i] == Agendum::Verbatim);
                lemma_verbatim_emission_agrees(source[i], emitted[i]);
            }
        };
        lemma_agreement_preserves_prefix(source, emitted, len, seed);
        return;
    }
    // The delimiters are respelled in the source marker family and run, so the
    // opener needs the masked read-back lemma and every other position agreement.
    // A closer's read-back relation collapses to agreement because it closes.
    if !compressible {
        assert(spec_delimiter_reads_back(source[0], emitted[0])) by {
            super::planner::lemma_block_agenda_index(len, release, true, false, 0);
            reveal(spec_is_outer_delimiter);
            reveal(spec_delimiter_agendum);
            assert(agenda[0] == Agendum::Normalized);
            lemma_normalized_emission_reads_back(source[0], emitted[0]);
        }
        assert(forall|i: int|
            #![trigger source[i]]
            1 <= i < len ==> spec_same_reading(source[i], #[trigger] emitted[i])) by {
            assert forall|i: int|
                #![trigger source[i]]
                1 <= i < len implies spec_same_reading(source[i], #[trigger] emitted[i]) by {
                if i == len - 1 && release == Release::Matched {
                    super::planner::lemma_block_agenda_index(len, release, true, false, i);
                    reveal(spec_is_outer_delimiter);
                    reveal(spec_delimiter_agendum);
                    assert(agenda[i] == Agendum::Normalized);
                    lemma_normalized_emission_reads_back(source[i], emitted[i]);
                    lemma_closer_reads_back_as_itself(opener, source[i], emitted[i]);
                } else {
                    super::planner::lemma_block_agenda_index(len, release, true, false, i);
                    reveal(spec_is_outer_delimiter);
                    reveal(spec_delimiter_agendum);
                    assert(agenda[i] == Agendum::Verbatim);
                    lemma_verbatim_emission_agrees(source[i], emitted[i]);
                }
            }
        };
        lemma_delimited_prefix_agreement(source, emitted, len, seed);
        return;
    }
    // A compressible block's delimiters are what change, and the interior is what
    // the change must leave alone. The contract supplies the written three-backtick
    // shape at each delimiter; the fold supplies the guarded-interior facts.
    reveal(spec_compressed);
    reveal(spec_closes);
    super::planner::lemma_block_agenda_index(len, release, true, true, 0);
    reveal(spec_is_outer_delimiter);
    reveal(spec_delimiter_agendum);
    assert(agenda[0] == Agendum::Compressed);
    lemma_compressed_emission_shape(source[0], emitted[0]);
    if release == Release::Matched {
        assert(forall|i: int|
            #![trigger source[i]]
            0 < i < len - 1 ==> spec_same_reading(source[i], #[trigger] emitted[i])) by {
            assert forall|i: int|
                #![trigger source[i]]
                0 < i < len - 1 implies spec_same_reading(source[i], #[trigger] emitted[i]) by {
                super::planner::lemma_block_agenda_index(len, release, true, true, i);
                reveal(spec_is_outer_delimiter);
                reveal(spec_delimiter_agendum);
                assert(agenda[i] == Agendum::Verbatim);
                lemma_verbatim_emission_agrees(source[i], emitted[i]);
            }
        };
        super::planner::lemma_block_agenda_index(len, release, true, true, len - 1);
        reveal(spec_is_outer_delimiter);
        reveal(spec_delimiter_agendum);
        assert(agenda[len - 1] == Agendum::Compressed);
        lemma_compressed_emission_shape(source[len - 1], emitted[len - 1]);
        assert(source[len - 1].trailing_blank);
        lemma_compressed_block_regions(seed, source, emitted, len, opener, true);
    } else {
        // An unterminated block's last line is interior, and its two runs end in
        // different states; the prefix lemma carries the regions and claims no
        // state, which is why the tail needs no following span.
        assert(forall|i: int|
            #![trigger source[i]]
            0 < i < len ==> spec_same_reading(source[i], #[trigger] emitted[i])) by {
            assert forall|i: int|
                #![trigger source[i]]
                0 < i < len implies spec_same_reading(source[i], #[trigger] emitted[i]) by {
                super::planner::lemma_block_agenda_index(len, release, true, true, i);
                reveal(spec_is_outer_delimiter);
                reveal(spec_delimiter_agendum);
                assert(agenda[i] == Agendum::Verbatim);
                lemma_verbatim_emission_agrees(source[i], emitted[i]);
            }
        };
        super::compression::lemma_compressed_prefix(seed, source, emitted, len, opener);
    }
}

} // verus!