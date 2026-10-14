//! The region half of the document theorem.
//!
//! [`lemma_released_block_reads_back`] is the per-block step the composition
//! consumes: every line a released block covers keeps the region it had. It is
//! the meeting point of both halves above, calling the agreement argument for
//! the lines that do not move and the compressed-block induction for the
//! delimiter that does.
//!
//! Extracted from `verus/plan_spec.rs` at a declaration seam so both files come
//! in under the 400-line ceiling `AGENTS.md` sets. The lemma was `pub(super)`
//! before the move and stays so, which is why the parent re-imports it rather
//! than re-exporting it.

use crate::fence_spec::{spec_closes, spec_compressed, spec_regions_seeded, spec_rewrite_permitted};
use crate::production_fence::{FenceState, LineFeatures};
use super::{lemma_agreement_preserves_prefix, spec_masked, spec_same_reading};
use super::{lemma_compressed_block_regions};
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
/// The hypothesis is the one the pass cannot prove about itself. The pass renders
/// an agendum into bytes and a regular expression reads those bytes back; Verus
/// cannot reason about a compiled regular expression, so what the emitted bytes
/// read as is supplied. The `requires` below supplies it directly, at the three
/// emission shapes a released block can produce: every line written verbatim
/// reads back as itself, every line written in its source spelling reads back as
/// itself, and the lines the pass compressed read back as three backticks at
/// their source depth. Round two on a compressed line is the case worth naming:
/// the bytes differ from the source line's, and the theorem is that the *region*
/// does not.
///
/// [`spec_emission_reads_back`] states the same three shapes in one predicate,
/// but it is *not* what this call site consumes: no `SpecLine` is constructed
/// anywhere in this file, so nothing can discharge that predicate, and the
/// hypotheses here are the shapes written out rather than an instantiation of
/// it. The two are kept in step by hand and the discrepancy is the reason the
/// predicate reads as documentation rather than as an obligation.
///
/// The conclusion names the whole block because the block is where the two
/// induction principles above live: interior lines are covered by
/// [`lemma_compressed_prefix`], and the two delimiters by
/// [`lemma_compressed_block_regions`]. Nothing here restates either.
pub proof fn lemma_released_block_reads_back(
    seed: Option<FenceState>,
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    len: int,
    opener: FenceState,
    closer: bool,
    compressible: bool,
)
    requires
        1 <= len,
        source.len() == len,
        emitted.len() == len,
        // The opener reads back as the compressed or the source spelling according
        // to the block's one answer, at the opener's own depth.
        source[0].marker == Some(opener.marker),
        source[0].marker_len == opener.marker_len,
        source[0].depth == opener.open_depth,
        emitted[0].depth == opener.open_depth,
        compressible ==> emitted[0].marker == Some('`') && emitted[0].marker_len == 3,
        !compressible ==> spec_same_reading(source[0], emitted[0]),
        spec_masked(seed, opener.open_depth).is_none(),
        // Every position after the opener reads back as the emitter wrote it: the
        // pass writes a non-delimiter position verbatim and a non-compressible
        // delimiter in its source spelling, and both are the same reading. The one
        // non-compressible delimiter this does not cover is a fallback whose `null`
        // specifier is dropped, since that raises `trailing_blank`; such a document
        // is excluded by this hypothesis rather than discharged by it, and
        // `docs/verification.md` records the exclusion and the relation
        // (`spec_delimiter_reads_back`) that would bring it inside.
        forall|i: int|
            #![trigger source[i]]
            0 < i < len - 1 ==> spec_same_reading(source[i], #[trigger] emitted[i]),
        forall|i: int|
            #![trigger source[i]]
            0 < i < len - 1 ==> source[i].depth >= opener.open_depth && !spec_closes(
                opener,
                source[i],
            ) && spec_rewrite_permitted(opener, source[i]),
        // The last position is the closer for a matched block and one more
        // interior line for an unterminated one. A matched closer is the one
        // position whose emitted reading is *not* its source reading when the block
        // is compressible, so the bytes it was written with are named here rather
        // than left to be inferred: the emitter writes it as three backticks at the
        // source line's own depth, and the closing relation to the compressed opener
        // follows from that. A non-compressible closer reads back as its source line
        // and is discharged by the must-follow below, exactly as the interior is.
        closer && compressible ==> spec_closes(opener, source[len - 1]),
        closer && compressible ==> 2 <= len,
        closer && compressible ==> emitted[len - 1].marker == Some('`'),
        closer && compressible ==> emitted[len - 1].marker_len == 3,
        closer && compressible ==> emitted[len - 1].depth == source[len - 1].depth,
        closer && compressible ==> emitted[len - 1].trailing_blank,
        closer && !compressible ==> spec_same_reading(source[len - 1], emitted[len - 1]),
        !closer ==> spec_same_reading(source[len - 1], emitted[len - 1]),
        !closer ==> source[len - 1].depth >= opener.open_depth && !spec_closes(
            opener,
            source[len - 1],
        ) && spec_rewrite_permitted(opener, source[len - 1]),
    ensures spec_regions_seeded(source, len, seed) == spec_regions_seeded(emitted, len, seed),
{
    // A non-compressible block is the agreement case throughout: the opener reads
    // back as itself and so does every later line, so the whole block is covered by
    // the agreement induction and needs no block reasoning at all.
    if !compressible {
        assert(forall|i: int|
            #![trigger source[i]]
            0 <= i < len ==> spec_same_reading(source[i], #[trigger] emitted[i])) by {
            assert forall|i: int|
                #![trigger source[i]]
                0 <= i < len implies spec_same_reading(source[i], #[trigger] emitted[i]) by {
                assert(source[0].marker == Some(opener.marker));
            }
        };
        lemma_agreement_preserves_prefix(source, emitted, len, seed);
        return;
    }
    // A compressible block's delimiters are what change, and the interior is what
    // the change must leave alone: the prefix lemma carries the interior and the
    // whole-block lemma carries the closer, so this is a call to each rather than a
    // second induction.
    reveal(spec_compressed);
    reveal(spec_closes);
    if closer {
        lemma_compressed_block_regions(seed, source, emitted, len, opener, true);
    } else {
        // An unterminated block has no closer, so its last line is interior: the
        // two must-follows supply what the prefix lemma wants, and the block lemma
        // is the unterminated arm of the same call.
        lemma_compressed_block_regions(seed, source, emitted, len, opener, false);
    }
}

} // verus!
