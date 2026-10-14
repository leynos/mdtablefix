//! Reading agreement between a source line and its emission.
//!
//! The easier half of the document theorem: the lines the planner leaves alone do
//! not move, and the region they occupy survives because their reading does.
//! [`spec_same_reading`] is the relation, [`lemma_agreement_preserves_prefix`]
//! is the pointwise argument applied to a whole prefix, and the two "reads
//! back" specifications name the cases where the emission is deliberately not
//! the source — the null language specifier, and the null-info fence-tag flip.
//!
//! Extracted from `verus/plan_spec.rs` at a declaration seam so both files come
//! in under the 400-line ceiling `AGENTS.md` sets. The items keep the
//! visibility they had before the move: the parent re-imports the three the
//! composition reaches, and the two private helper lemmas stay private to this
//! module.

use crate::fence_spec::{spec_closes, spec_fence_next, spec_fence_region, spec_regions_seeded, spec_state_seeded};
use crate::production_fence::{FenceState, LineFeatures};
use crate::production_plan::{Agendum};
use vstd::prelude::*;

verus! {

/// One source line together with the reading of the bytes the pass writes.
///
/// The two readings are not redundant. `source` is what the kernel's transition
/// consumed while the planner decided what to do with the line; `emitted` is
/// what the same transition will consume on the next pass, once the emitter has
/// written the agendum the planner chose. The document theorem is exactly the
/// statement that these two agree, so a specification carrying only one of them
/// would have nothing to prove.
pub struct SpecLine {
    /// The kernel's view of the source line.
    pub source: LineFeatures,
    /// The kernel's view of the bytes the pass writes for the line.
    pub emitted: LineFeatures,
    /// Whether the line carries a delimiter rewrite the emitter could write.
    pub rewritable: bool,
    /// Whether the emitter writes the line in its source marker family and run,
    /// with only a null language specifier dropped.
    pub normalizable: bool,
    /// Whether the emitter writes the line byte for byte.
    pub verbatim: bool,
}

/// Whether two readings are the same reading.
///
/// A [`LineFeatures`] value has no field the transition ignores, so this is
/// structural equality. Naming it keeps the lemmas below reading as statements
/// about *readings* rather than about a four-tuple.
pub open spec fn spec_same_reading(a: LineFeatures, b: LineFeatures) -> bool {
    a == b
}

/// Whether the emitted reading of a line agrees with its source reading.
///
/// Naming the projection keeps the lemmas below from having to write the two
/// field projections out; a rule stated in terms of a `SpecLine` can then be
/// about `spec_line_agrees(line)` without spelling the pair.
pub open spec fn spec_line_agrees(line: SpecLine) -> bool {
    spec_same_reading(line.source, line.emitted)
}

/// What re-spelling a delimiter in its own marker family and run guarantees.
///
/// This is the relation the `Preserve` spelling establishes, and it is strictly
/// weaker than agreement: the marker family, the run length, and the depth are
/// kept, and the one change the spelling may make is to drop a language
/// specifier that trims to `null`. Dropping it turns a non-blank tag blank, so
/// the emitted reading is the source reading with `trailing_blank` possibly
/// raised — never lowered, because a specifier is only ever removed.
///
/// The weakening is not a convenience. A null tag is a non-blank info string,
/// and `trailing_blank` *is* the grammar's blank-tag test: `spec_closes`
/// consults it, so the two readings disagree about whether such a line closes a
/// matching fence at its own depth. Stating plain agreement here would assert
/// something production does not do — `tests::fences::preserves_null_language_during_interior_conflict`
/// pins the rewrite `"````null"` to `"````"`.
pub open spec fn spec_delimiter_reads_back(source: LineFeatures, emitted: LineFeatures) -> bool {
    spec_same_reading(source, emitted) || spec_blank_tag_raised(source, emitted)
}

/// Whether `emitted` is `source` with nothing changed but the blank tag.
///
/// The tag may only be raised, never lowered: the spelling removes a specifier,
/// and a line with no specifier is not one this rewrite is applied to. Family,
/// run, and depth are carried across unchanged, which is what lets the region
/// and state lemmas below read the two lines as the same delimiter.
pub open spec fn spec_blank_tag_raised(source: LineFeatures, emitted: LineFeatures) -> bool {
    emitted.depth == source.depth && emitted.marker == source.marker && emitted.marker_len
        == source.marker_len && !source.trailing_blank && emitted.trailing_blank
}

/// The fence state a line is read against once a depth drop has been applied.
///
/// Both `spec_fence_next` and `spec_fence_region` begin with this test: a line
/// below the open fence's depth ends the fence before it is read, whatever the
/// line is. Factoring it out lets the lemmas below say "this line arrives with
/// no fence in force" without restating the transition's first branch.
pub open spec fn spec_masked(state: Option<FenceState>, depth: usize) -> Option<FenceState> {
    match state {
        Some(open) if depth < open.open_depth => None,
        other => other,
    }
}

/// The raw-text contract the document theorem was intended to be proved from.
///
/// The pass does not reason about text; it renders one of three agendas into
/// bytes, and a separate regular expression reads those bytes back on the next
/// pass. Verus cannot and should not reason about a compiled regular expression,
/// so the relationship between the two is an external contract, recorded here as
/// a predicate and in the verification ledger as an assumed boundary.
///
/// **This contract is not enforced and nothing consumes it.** Its only
/// occurrence in the tree is this definition, and its input type `SpecLine` is
/// never constructed, so no caller can supply it a subject. Seeding the
/// `Normalized` arm below to `true` verifies cleanly at 178 verified, 0 errors,
/// byte-identical to the unmutated control. Read the clauses as a statement of
/// what the frontier *should* establish, not as an assumption any theorem below
/// rests on; closing it means giving it a real consumer.
///
/// The contract says what the parser *reads off* the bytes the emitter writes,
/// and never that those bytes preserve a region, which is the thing being
/// proved. Each clause is a claim a counterexample could falsify at the
/// frontier. A verbatim write that was not byte-identical would break the first;
/// a normalization that changed the marker run or turned a blank tag non-blank
/// would break the second; a compressed write that did not carry the target
/// marker and run at the source depth would break the third. The production
/// regex and the production emitter are what hold it to account, and the
/// property tests over both are the evidence.
pub open spec fn spec_emission_reads_back(agendum: Agendum, line: SpecLine) -> bool {
    match agendum {
        // Written byte for byte, so it reads back as itself.
        Agendum::Verbatim => line.verbatim ==> spec_line_agrees(line),
        // Written with the source marker family and run, and with only a null
        // language specifier dropped. Dropping one raises the blank tag and
        // changes nothing else — `spec_delimiter_reads_back` is that relation,
        // and it is deliberately not plain agreement, which this spelling does
        // not establish.
        Agendum::Normalized => {
            line.normalizable ==> spec_delimiter_reads_back(line.source, line.emitted)
        },
        // Written as three backticks at the line's own depth. The pass rewrites
        // only a line the fence grammar read as a delimiter, so the written
        // bytes are read back as a delimiter of the target family and run.
        Agendum::Compressed => line.rewritable ==> (
            line.emitted.marker == Some('`') && line.emitted.marker_len == 3
                && line.emitted.depth == line.source.depth
        ),
    }
}

// ---------------------------------------------------------------------------

/// Two readings that agree give the same region, whatever the fence state.
///
/// This is the whole of the "left alone" argument, and it is total: it holds for
/// every state, including states the masks above would discard.
pub proof fn lemma_same_reading_preserves_region(
    state: Option<FenceState>,
    source: LineFeatures,
    emitted: LineFeatures,
)
    requires spec_same_reading(source, emitted),
    ensures spec_fence_region(state, source) == spec_fence_region(state, emitted),
{
}

/// Two readings that agree leave the same fence state behind, whatever it was.
///
/// Together with the region lemma this is what lets an induction over a document
/// compare two runs line by line: neither the classification nor the state can
/// diverge at a position where the two readings agree.
pub proof fn lemma_same_reading_preserves_next(
    state: Option<FenceState>,
    source: LineFeatures,
    emitted: LineFeatures,
)
    requires spec_same_reading(source, emitted),
    ensures spec_fence_next(state, source) == spec_fence_next(state, emitted),
{
}

/// A delimiter re-spelled in its own family preserves region and state, provided
/// no fence is in force at its own depth.
///
/// The guard is exactly the state a written outer delimiter is read in, and it
/// is what makes the blank-tag weakening harmless: with no fence in force the
/// transition takes its "None" branch, where a marker line opens a fence from
/// its own family and run alone. The blank tag is consulted only on the "Some"
/// branch, by `spec_closes`, and the guard is what excludes it.
pub(super) proof fn lemma_delimiter_reads_back_preserves(
    seed: Option<FenceState>,
    source: LineFeatures,
    emitted: LineFeatures,
)
    requires
        spec_delimiter_reads_back(source, emitted),
        spec_masked(seed, source.depth).is_none(),
    ensures
        spec_fence_region(seed, source) == spec_fence_region(seed, emitted),
        spec_fence_next(seed, source) == spec_fence_next(seed, emitted),
{
    reveal(spec_delimiter_reads_back);
    reveal(spec_blank_tag_raised);
}

/// The guard above is necessary, not decorative.
///
/// Under a matching fence at the delimiter's own depth the raised blank tag is a
/// closer where the source line was interior content. Without the guard the
/// previous lemma would be false, so the hypothesis cannot be dropped.
pub(super) proof fn lemma_blank_tag_flip_needs_the_guard()
    ensures
        spec_masked(Some(FenceState { marker: '`', marker_len: 3, open_depth: 0 }), 0).is_some(),
        spec_fence_region(
            Some(FenceState { marker: '`', marker_len: 3, open_depth: 0 }),
            LineFeatures { depth: 0, marker: Some('`'), marker_len: 4, trailing_blank: true },
        ) != spec_fence_region(
            Some(FenceState { marker: '`', marker_len: 3, open_depth: 0 }),
            LineFeatures { depth: 0, marker: Some('`'), marker_len: 4, trailing_blank: false },
        ),
{
    reveal(spec_fence_region);
    reveal(spec_fence_next);
    reveal(spec_closes);
    reveal(spec_masked);
}

/// `LEM-DOCUMENT-REGIONS-PRESERVED-BY-AGREEMENT`.
///
/// If every line of a prefix reads back as the line it was written from, the
/// prefix classifies identically in both runs and leaves the same fence state
/// behind. The result holds for every prefix, which is what a document-level
/// induction over the pass needs, and it needs no hypothesis about blocks,
/// releases, or markers: agreement is enough on its own.
///
/// This covers every document the pass writes without a compressed delimiter —
/// including every abandoned block, and every matched block whose endpoints are
/// not both rewrite candidates. It does **not** reach a conflict-bearing block
/// whose delimiter carries a `null` language specifier: such a block also takes
/// the source spelling, but dropping the specifier raises `trailing_blank`, so
/// its delimiter fails this lemma's `spec_same_reading` hypothesis. Those
/// hypotheses are unsatisfiable for a document
/// `tests::fences::preserves_null_language_during_interior_conflict` proves
/// reachable through `compress_fences`; restating them over
/// [`spec_delimiter_reads_back`] is what would bring that case inside.
pub proof fn lemma_agreement_preserves_prefix(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    upto: int,
    seed: Option<FenceState>,
)
    requires
        0 <= upto <= source.len(),
        emitted.len() == source.len(),
        forall|i: int|
            #![trigger source[i]]
            0 <= i < upto ==> spec_same_reading(source[i], #[trigger] emitted[i]),
    ensures
        spec_state_seeded(source, upto, seed) == spec_state_seeded(emitted, upto, seed),
        spec_regions_seeded(source, upto, seed) == spec_regions_seeded(emitted, upto, seed),
    decreases upto,
{
    if upto > 0 {
        lemma_agreement_preserves_prefix(source, emitted, upto - 1, seed);
        lemma_same_reading_preserves_region(
            spec_state_seeded(source, upto - 1, seed),
            source[upto - 1],
            emitted[upto - 1],
        );
        lemma_same_reading_preserves_next(
            spec_state_seeded(source, upto - 1, seed),
            source[upto - 1],
            emitted[upto - 1],
        );
    }
}

} // verus!
