//! Consumers of the emission contract.
//!
//! [`spec_emission_reads_back`] states what the parser reads off the bytes the
//! emitter writes, per agenda. A contract with no consumer is a comment, and
//! until this module existed the `Normalized` arm could be replaced by `true`
//! with the tree still verifying. Each lemma here extracts one clause and hands
//! it to the block and composition lemmas as a usable fact, so weakening an arm
//! breaks a *theorem's* postcondition instead of going unnoticed.
//!
//! The three agenda are the three spellings the pass can write, and each lemma
//! is shaped for the argument that consumes it: agreement for a verbatim line,
//! the weaker delimiter read-back for a normalized one, and the three-backtick
//! shape for a compressed one. [`lemma_closer_reads_back_as_itself`] is the
//! bridge that lets a normalized block treat its closer with its interior.
//!
//! Declared as a private child of `verus/plan_spec.rs` and re-exported through
//! it, so the mutation mirror and `verus/lib.rs` reach these names by the same
//! route as the contract they consume.

use crate::fence_spec::{spec_closes, spec_regions_seeded, spec_state_seeded};
use crate::production_fence::{FenceState, LineFeatures};
use crate::production_plan::Agendum;
use super::{
    spec_agendum_line, spec_blank_tag_raised, spec_delimiter_reads_back, spec_emission_reads_back,
    spec_line_agrees, spec_masked, spec_same_reading, spec_specline,
};
use vstd::prelude::*;

verus! {

/// A verbatim write reads back as its own source.
///
/// The first consumer, and the plainest: the emitter copies the line, so the
/// emitted reading is the source reading and the agreement lemma applies to it.
/// The hypothesis is stated over [`spec_agendum_line`] rather than over a
/// caller-built [`super::SpecLine`] so that a caller cannot claim the verbatim
/// spelling for a line the driver decided to rewrite.
pub proof fn lemma_verbatim_emission_agrees(
    source: LineFeatures,
    emitted: LineFeatures,
)
    requires spec_emission_reads_back(
        Agendum::Verbatim,
        spec_agendum_line(source, emitted, Agendum::Verbatim),
    ),
    ensures spec_same_reading(source, emitted),
{
    reveal(spec_emission_reads_back);
    reveal(spec_agendum_line);
    reveal(spec_specline);
    reveal(spec_line_agrees);
}

/// A normalized write reads back in the source marker family and run.
///
/// The load-bearing consumer for the internal `null`-specifier case: the
/// relation it extracts is deliberately weaker than agreement, so a document
/// whose conflict fallback drops a `null` tag still satisfies it. Weakening the
/// `Normalized` arm of the contract to `true` makes this lemma's postcondition
/// unprovable, which is what the emission mutation gate of Task 2 relies on —
/// the failure lands on this lemma, not on a witness that could be deleted
/// without touching the theorem.
pub proof fn lemma_normalized_emission_reads_back(
    source: LineFeatures,
    emitted: LineFeatures,
)
    requires spec_emission_reads_back(
        Agendum::Normalized,
        spec_agendum_line(source, emitted, Agendum::Normalized),
    ),
    ensures spec_delimiter_reads_back(source, emitted),
{
    reveal(spec_emission_reads_back);
    reveal(spec_agendum_line);
    reveal(spec_specline);
}

/// A compressed write reads back as three backticks at the source depth.
///
/// It supplies the opener shape the compressed-block lemma needs, and the
/// no-lowering half of the blank tag that lets a compressed closer still close.
/// Both are properties of the *written bytes*, so both have to come from the
/// contract rather than from any reasoning about the transition.
pub proof fn lemma_compressed_emission_shape(
    source: LineFeatures,
    emitted: LineFeatures,
)
    requires spec_emission_reads_back(
        Agendum::Compressed,
        spec_agendum_line(source, emitted, Agendum::Compressed),
    ),
    ensures
        emitted.marker == Some('`'),
        emitted.marker_len == 3,
        emitted.depth == source.depth,
        source.trailing_blank ==> emitted.trailing_blank,
{
    reveal(spec_emission_reads_back);
    reveal(spec_agendum_line);
    reveal(spec_specline);
}

/// A delimiter that closes under the source reading agrees with its emission.
///
/// The blank-tag weakening in [`spec_delimiter_reads_back`] cannot apply to a
/// line that closes a block: [`spec_closes`] requires the line's blank tag, and
/// the weakening *raises* that tag from a non-blank source. So a closing
/// delimiter's read-back relation collapses to agreement, which is what lets a
/// normalized block treat its closer with the same lemma as its interior.
pub proof fn lemma_closer_reads_back_as_itself(
    opener: FenceState,
    source: LineFeatures,
    emitted: LineFeatures,
)
    requires
        spec_closes(opener, source),
        spec_delimiter_reads_back(source, emitted),
    ensures spec_same_reading(source, emitted),
{
    reveal(spec_delimiter_reads_back);
    reveal(spec_blank_tag_raised);
    reveal(spec_closes);
}

/// `LEM-DELIMITED-PREFIX-AGREEMENT` — one respelled delimiter, then agreement.
///
/// The normalized-block argument in one lemma: the block's opener is re-spelled
/// in its own marker family and run (so it needs the masked read-back lemma),
/// and every later line reads back as itself (so agreement carries the rest).
/// Together they make the two runs classify identically and leave the same fence
/// state behind, which is what the released-block lemma needs for the
/// conflict-fallback route — the route whose `null` specifier makes the opener
/// settle for the weaker relation.
///
/// The induction peels the last line. At length one the opener is the whole
/// prefix and the read-back lemma decides it; past that the last line is one of
/// the agreeing ones, so the same-region and same-next lemmas extend the
/// induction's result by it.
pub proof fn lemma_delimited_prefix_agreement(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    len: int,
    seed: Option<FenceState>,
)
    requires
        1 <= len,
        len <= source.len(),
        emitted.len() == source.len(),
        spec_delimiter_reads_back(source[0], emitted[0]),
        spec_masked(seed, source[0].depth).is_none(),
        forall|i: int|
            #![trigger source[i]]
            1 <= i < len ==> spec_same_reading(source[i], #[trigger] emitted[i]),
    ensures
        spec_state_seeded(source, len, seed) == spec_state_seeded(emitted, len, seed),
        spec_regions_seeded(source, len, seed) == spec_regions_seeded(emitted, len, seed),
    decreases len,
{
    if len == 1 {
        super::agreement::lemma_delimiter_reads_back_preserves(seed, source[0], emitted[0]);
        // The one-line run's single region and the state it leaves behind are
        // both read off the first line, so unfolding the two seeded definitions
        // twice reaches their zero-length bases.
        reveal_with_fuel(spec_state_seeded, 2);
        reveal_with_fuel(spec_regions_seeded, 2);
    } else {
        lemma_delimited_prefix_agreement(source, emitted, len - 1, seed);
        let before = spec_state_seeded(source, len - 1, seed);
        assert(spec_state_seeded(emitted, len - 1, seed) == before);
        assert(spec_same_reading(source[len - 1], emitted[len - 1]));
        super::lemma_same_reading_preserves_region(before, source[len - 1], emitted[len - 1]);
        super::lemma_same_reading_preserves_next(before, source[len - 1], emitted[len - 1]);
        reveal_with_fuel(spec_state_seeded, 2);
        reveal_with_fuel(spec_regions_seeded, 2);
    }
}

} // verus!