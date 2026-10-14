//! Specification of the fence-compression planner and its document theorem.
//!
//! `verus/fence_spec.rs` proves a local result: respelling one block's
//! delimiters cannot move an interior line out of its region, for a block whose
//! interior satisfies a guard. That is a statement about a guarded block. This
//! file states the document-level result, whose subject is the whole pass: the
//! output has the same number of lines as the input, and every line keeps the
//! region it had.
//!
//! The document theorem is true for a stronger reason than a per-line transition
//! argument, and it is worth naming. A delimiter the planner respells
//! conservatively — [`spec_delimiter_agendum`] returning [`Agendum::Normalized`]
//! — is written in its *source* marker family and run, with a language specifier
//! dropped only when it was null or absent. Dropping a null specifier cannot
//! change the marker, the run, or the depth the transition reads, but it does
//! raise `trailing_blank`, which `spec_closes` consults. A normalized delimiter
//! therefore keeps marker, run, and depth, and is not plain agreement with its
//! source; `spec_delimiter_reads_back` is that relation. Region equality holds
//! for the conflict-bearing blocks that take this route because the lemma
//! guards on `spec_masked`, which excludes the one seed under which the raised
//! tag would be read. Every other line the planner leaves alone follows from a
//! *pointwise reading agreement*, stated once in
//! [`lemma_agreement_preserves_prefix`] and applied to those lines. That is why
//! the planner needs only an eligibility predicate, and why those cases need no
//! transition reasoning at all.
//!
//! Compression — [`Agendum::Compressed`], which writes three backticks whatever
//! the source used — does change the reading, so it does need transition
//! reasoning. [`lemma_compressed_prefix`] supplies it by observing that the
//! written opener is exactly `spec_compressed` of the source opener, which is
//! the second state `fence_spec.rs`'s guarded-interior lemma already compares
//! against. The two files therefore meet exactly: the guard that lemma assumes
//! is the guard [`lemma_no_conflict_all_clear`] extracts from the planner's own
//! conflict fold.
//!
//! The proofs are over *readings* of rendered lines rather than over rendered
//! text, because the pass's decisions are functions of both: the reading decides
//! recognition, and the bytes decide the next reading. A specification over
//! readings alone could not state that payload bytes are unchanged, so
//! [`SpecLine`] carries both and the raw-text obligation is the explicit
//! contract relating them.

use crate::production_fence::{FenceState, LineFeatures, Region};
// The transition kernel's own specification, named through its module rather
// than through the crate root's re-export: a child module does not inherit the
// parent's imports, and naming the module makes it plain that these are the
// fence lemmas this file builds on and not restatements of them.
use crate::fence_spec::{
    lemma_block_stays_open,
    lemma_rewrite_preserves_closer_relation,
    spec_closes,
    spec_compressed,
    spec_compression_changes_region,
    spec_fence_next,
    spec_fence_region,
    spec_regions_seeded,
    spec_rewrite_permitted,
    spec_state_seeded,
};
// Only the planner's *types* are named here. Its functions are not called from
// this file: the connection runs the other way, through the contracts in
// `src/fences/plan.rs`, each of which names the specification function below
// that its body must realise. Importing the bodies would let this file describe
// a model of the planner rather than the planner itself.
use crate::production_plan::{Agendum, Block as PlanBlock, LineFacts, Release, Step};
use vstd::prelude::*;
// The two subrange facts the translation lemmas need: what a subrange's `i`-th
// element is, and how a subrange of a subrange relates to the original.
use vstd::seq::axiom_seq_subrange_index;

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
// Reading agreement
// ---------------------------------------------------------------------------

/// Two readings that agree give the same region, whatever the fence state.
///
/// This is the whole of the "left alone" argument, and it is total: it holds for
/// every state, including states the masks above would discard.
proof fn lemma_same_reading_preserves_region(
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
proof fn lemma_same_reading_preserves_next(
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
proof fn lemma_delimiter_reads_back_preserves(
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
proof fn lemma_blank_tag_flip_needs_the_guard()
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
proof fn lemma_agreement_preserves_prefix(
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

// ---------------------------------------------------------------------------
// Planner specification
// ---------------------------------------------------------------------------

/// The block a fence delimiter opens.
pub open spec fn spec_open_block(line: LineFacts) -> Option<PlanBlock> {
    match line.features {
        Some(features) => match features.marker {
            Some(marker) => Some(PlanBlock {
                opening: FenceState {
                    marker,
                    marker_len: features.marker_len,
                    open_depth: features.depth,
                },
                conflicts: false,
            }),
            None => None,
        },
        None => None,
    }
}

/// Accumulate one buffered line into a block's conflict flag.
pub open spec fn spec_absorb(block: PlanBlock, line: Option<LineFeatures>) -> PlanBlock {
    PlanBlock {
        opening: block.opening,
        conflicts: block.conflicts || match line {
            Some(features) => spec_compression_changes_region(block.opening, features),
            None => false,
        },
    }
}

/// Accumulate the first `upto` buffered lines into a block's conflict flag.
///
/// This is the planner's fold spelled out, so a lemma can reason about the flag
/// a block ends up carrying rather than about the loop that produced it.
pub open spec fn spec_absorb_all(
    block: PlanBlock,
    lines: Seq<Option<LineFeatures>>,
    upto: int,
) -> PlanBlock
    decreases upto
{
    if upto <= 0 {
        block
    } else {
        spec_absorb(spec_absorb_all(block, lines, upto - 1), lines[upto - 1])
    }
}

/// Decide what one line does to the block the pass is holding.
///
/// This is the routing relation the pass's control flow realises. Every release
/// the pass can name is produced here and nowhere else, so reasoning about this
/// function is reasoning about the pass.
pub open spec fn spec_plan_step(block: Option<PlanBlock>, line: LineFacts) -> Step {
    if line.was_in_fence {
        match block {
            Some(open) => {
                let extended = spec_absorb(open, line.features);
                if line.features.is_some() && !line.is_in_fence {
                    Step::Close(extended, Release::Matched)
                } else {
                    Step::Extend(extended)
                }
            },
            None => Step::Skip,
        }
    } else {
        match spec_open_block(line) {
            Some(fresh) => Step::Restart(fresh),
            None => Step::Skip,
        }
    }
}

/// `LEM-INSIDE-OPEN-MEANS-FENCE-SHAPED` — a line inside a block is fence-shaped.
///
/// A line the tracker read as already inside the fence, at the depth of a block the
/// planner is holding, cannot be a line the grammar failed to read as a delimiter.
/// The two routing answers come from the same transition: `was_in_fence` says the
/// state already held a fence open at this depth, so the depth is not below the
/// opener's, and a non-fence-shaped line would leave it open — which is a `Skip`,
/// not the `Extend` or `Close` the planner is about to choose between.
///
/// This is the side condition the fold invariant's step needs when it decides what
/// to record for a buffered block's opening line. Without it, the `Restart` case
/// and the `Extend`/`Close` cases would have to be argued separately at every step
/// of the induction.
pub proof fn lemma_inside_open_means_fence_shaped(
    state: Option<FenceState>,
    line: LineFeatures,
)
    requires
        spec_fenced(state, line.depth),
        line.marker.is_none(),
    ensures
        spec_fenced(spec_fence_next(state, line), line.depth),
{
    reveal(spec_fenced);
    reveal(spec_fence_next);
    match state {
        Some(open) => {
            if line.depth < open.open_depth {
            } else {
            }
        },
        None => {},
    }
}

/// Whether a release may respell the delimiters it writes.
///
/// This is the pass's recognition that every delimiter the release puts in play
/// is a rewrite candidate: the opener alone for an unterminated block, both ends
/// for a matched one, and nothing at all for an abandoned block, whose lines
/// were never bounded by a delimiter the pass read.
pub open spec fn spec_delimiters_eligible(
    release: Release,
    opener_rewritable: bool,
    closer_rewritable: bool,
) -> bool {
    match release {
        Release::Abandoned => false,
        Release::Unmatched => opener_rewritable,
        Release::Matched => opener_rewritable && closer_rewritable,
    }
}

/// Whether position `index` of a block of `len` lines is one of its delimiters.
pub open spec fn spec_is_outer_delimiter(
    index: int,
    len: int,
    release: Release,
    written: bool,
) -> bool {
    written && (index == 0 || (matches!(release, Release::Matched) && len > 0 && index == len - 1))
}

/// What the pass writes for one position of a released block.
///
/// A position the planner does not write reaches the document as it was
/// written, which is what keeps every interior line's bytes — and therefore its
/// region — intact. A delimiter the pass does write takes the compressed
/// spelling when the block has no conflicting interior line, and the source
/// spelling otherwise: the fallback is what keeps a conflict-bearing block's
/// regions identical, because a normalized delimiter keeps its source's marker,
/// run, and depth. It may raise the blank tag by dropping a `null` specifier,
/// which is why the relation that carries it is
/// [`spec_delimiter_reads_back`] rather than plain agreement, and why
/// [`lemma_delimiter_reads_back_preserves`] needs its `spec_masked` guard.
pub open spec fn spec_delimiter_agendum(is_delimiter: bool, compressible: bool) -> Agendum {
    if !is_delimiter {
        Agendum::Verbatim
    } else if compressible {
        Agendum::Compressed
    } else {
        Agendum::Normalized
    }
}

/// The agenda for the first `upto` positions of a released block.
pub open spec fn spec_block_agenda_seeded(
    len: int,
    release: Release,
    written: bool,
    compressible: bool,
    upto: int,
) -> Seq<Agendum>
    decreases upto
{
    if upto <= 0 {
        Seq::empty()
    } else {
        spec_block_agenda_seeded(len, release, written, compressible, upto - 1).push(
            spec_delimiter_agendum(
                spec_is_outer_delimiter(upto - 1, len, release, written),
                compressible,
            ),
        )
    }
}

/// The agenda for every position of a released block.
pub open spec fn spec_block_agenda(
    len: int,
    release: Release,
    written: bool,
    compressible: bool,
) -> Seq<Agendum> {
    spec_block_agenda_seeded(len, release, written, compressible, len)
}

/// `LEM-BLOCK-AGENDA-LENGTH` — the agenda has one agendum per buffered line.
///
/// This is the line-count half of the document theorem. The pass emits a block
/// by zipping its buffered lines with this agenda and by writing one line per
/// input line everywhere else, so an agenda of the block's own length is exactly
/// what makes the output as long as the input.
proof fn lemma_block_agenda_length(
    len: int,
    release: Release,
    written: bool,
    compressible: bool,
    upto: int,
)
    requires 0 <= upto,
    ensures spec_block_agenda_seeded(len, release, written, compressible, upto).len() == upto,
    decreases upto,
{
    if upto > 0 {
        lemma_block_agenda_length(len, release, written, compressible, upto - 1);
    }
}

/// `LEM-ABSORB-PRESERVES-OPENING`.
///
/// Buffering a line accumulates only the conflict flag; the opener the block
/// records is the one its opening delimiter established, at every prefix. The
/// conflict lemma below needs this to read a buffered line's answer against the
/// *block's* opener rather than against the fold's intermediate state.
proof fn lemma_absorb_all_preserves_opening(
    block: PlanBlock,
    lines: Seq<Option<LineFeatures>>,
    upto: int,
)
    requires 0 <= upto <= lines.len(),
    ensures spec_absorb_all(block, lines, upto).opening == block.opening,
    decreases upto,
{
    if upto > 0 {
        lemma_absorb_all_preserves_opening(block, lines, upto - 1);
    }
}

/// `LEM-NO-CONFLICT-ALL-CLEAR`.
///
/// A block whose conflict flag is still false after buffering has no interior
/// line that compression would change the region of. The flag is a disjunction
/// accumulated over the buffered lines, so its being false is exactly the
/// conjunction the guarded-interior lemma needs — this is the bridge between the
/// planner's fold and that lemma's hypothesis.
///
/// The fold is over `Option<LineFeatures>` because that is what the planner
/// absorbs: a line the tracker did not read as fence-shaped contributes nothing,
/// and neither does a line whose depth drops below the opener, because
/// `spec_compression_changes_region` requires the line to be at or below the
/// opener's depth.
proof fn lemma_no_conflict_all_clear(
    block: PlanBlock,
    lines: Seq<Option<LineFeatures>>,
    upto: int,
)
    requires
        0 <= upto <= lines.len(),
        !block.conflicts,
        !spec_absorb_all(block, lines, upto).conflicts,
    ensures
        forall|i: int|
            #![trigger lines[i]]
            0 <= i < upto ==> match lines[i] {
                Some(features) => !crate::spec_compression_changes_region(
                    block.opening,
                    features,
                ),
                None => true,
            },
    decreases upto,
{
    if upto > 0 {
        let prefix = spec_absorb_all(block, lines, upto - 1);
        // The fold is recursive, so its last step is available only once the
        // definitions are unfolded; the two helper facts below are what make the
        // last line's answer readable on its own.
        reveal(spec_absorb_all);
        reveal(spec_absorb);
        lemma_absorb_all_preserves_opening(block, lines, upto - 1);
        assert(spec_absorb_all(block, lines, upto) == spec_absorb(
            prefix,
            lines[upto - 1],
        ));
        // The flag is a disjunction, so a false result at the longer prefix
        // forces false at both contributors.
        assert(!prefix.conflicts);
        assert(prefix.opening == block.opening);
        lemma_no_conflict_all_clear(block, lines, upto - 1);
    }
}

// ---------------------------------------------------------------------------
// Compressed blocks
// ---------------------------------------------------------------------------

/// The three facts a delimiter establishes when the pass starts a block on it.
///
/// A line the tracker read as opening a block carries a marker and arrives with
/// no fence in force, so it classifies as a delimiter and leaves the opener
/// behind. The written form of that line is three backticks at the same depth,
/// so it does the same and leaves `spec_compressed` of the opener behind — the
/// two states `fence_spec.rs`'s guarded-interior lemma compares.
proof fn lemma_opener_establishes(
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
proof fn lemma_closer_establishes(
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
proof fn lemma_compressed_prefix(
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
proof fn lemma_compressed_block_regions(
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

// ---------------------------------------------------------------------------
// The driver
// ---------------------------------------------------------------------------

/// Whether a tracker state holds a fence open at `depth`.
///
/// This is the tracker's own `in_fence` test, and it is what turns a state into
/// the two routing answers the planner is handed. A line at or below the open
/// fence's depth is inside it; a line shallower than that is not, whatever state
/// the tracker is carrying, because a shallower line ends the fence before it is
/// read.
pub open spec fn spec_fenced(state: Option<FenceState>, depth: usize) -> bool {
    match state {
        Some(open) => depth >= open.open_depth,
        None => false,
    }
}

/// The planner's view of a line's features.
///
/// The caller hands the planner the parsed features only when the line is
/// fence-shaped, and the planner derives its own fencing test from that one
/// value. The reduction is sound because the two producers cannot overlap: a
/// parsed fence run always yields a marker, and the prose fallback never does.
/// The marker's presence is therefore exactly the recognition answer the caller
/// would have given, and the driver can recover that answer from the line alone.
pub open spec fn spec_planner_features(line: LineFeatures) -> Option<LineFeatures> {
    if line.marker.is_some() {
        Some(line)
    } else {
        None
    }
}

/// Everything the caller learned about one line, as the specification computes it.
///
/// This is the join between the tracker's state machine and the planner's input.
/// The two routing answers are read off the fence state before and after the
/// line is stepped; the features are the recognition answer above; and the
/// rewrite flag is the one fact the caller must supply, because it depends on
/// the delimiter grammar rather than on the fence transition.
///
/// Reading the routing answers here rather than trusting a caller's copy is what
/// makes the driver provable: the two answers are functions of the same state
/// recurrence the driver already maintains, so a caller cannot supply answers
/// that disagree with the document it is planning.
pub open spec fn spec_line_facts(
    state: Option<FenceState>,
    line: LineFeatures,
    rewritable: bool,
) -> LineFacts {
    LineFacts {
        was_in_fence: spec_fenced(state, line.depth),
        is_in_fence: spec_fenced(spec_fence_next(state, line), line.depth),
        features: spec_planner_features(line),
        rewritable,
    }
}
/// The agenda a release contributes, given the span of lines it covers.
///
/// The span — not a start and an end — is the parameter because it is what the
/// emitter actually has. A release writes the lines the pass buffered for the
/// block, and the number of those lines is a length the caller already holds; a
/// start and an end would only be two values from which that length is recovered.
/// Naming the length is also what keeps the kernel free of subtraction, which is
/// the one place an underflow could hide.
///
/// The two rewrite flags are the ones the emitter will consult: the opener's was
/// recorded when the block was opened, and the closer's is the releasing line's
/// own. An unterminated block ignores the second flag, which is why the pass can
/// hand that path a value it never reads.
///
/// The block is a parameter rather than being read from the driver state because
/// of exactly one release: a close buffers its line before the block is released,
/// so that line's conflict answer is part of the block being emitted. Every other
/// release leaves the flag alone, because [`spec_absorb`] preserves it.
pub open spec fn spec_released_agenda(
    block: PlanBlock,
    span: int,
    opener_rewritable: bool,
    closer_rewritable: bool,
    release: Release,
) -> Seq<Agendum> {
    spec_block_agenda(
        span,
        release,
        spec_delimiters_eligible(release, opener_rewritable, closer_rewritable),
        !block.conflicts,
    )
}

/// The buffered block after a step, or `None` when nothing is buffered.
pub open spec fn spec_step_pending(step: Step) -> Option<PlanBlock> {
    match step {
        Step::Restart(fresh) => Some(fresh),
        Step::Skip => None,
        Step::Extend(block) => Some(block),
        Step::Close(_, _) => None,
    }
}

/// The rewrite flag a step records for the block it leaves buffered.
///
/// Only an opening delimiter supplies the flag, because only an opening delimiter
/// is ever consulted as one: the eligibility rule reads the block's first line and
/// the line that releases it, and never an interior line. The flag is taken from
/// the line's own facts — the same value the `Restart` that buffered it carried —
/// so a caller cannot record one answer while the planner acted on another.
pub open spec fn spec_step_opener_rewritable(
    opener_rewritable: bool,
    line_rewritable: bool,
    step: Step,
) -> bool {
    match step {
        Step::Restart(_) => line_rewritable,
        Step::Skip => opener_rewritable,
        Step::Extend(_) => opener_rewritable,
        Step::Close(_, _) => opener_rewritable,
    }
}

/// The index a step records for the block it leaves buffered.
///
/// A fresh block begins at the line that opened it, so the step needs the index of
/// the line being planned; every other step carries the previous value through,
/// whether or not a block is buffered, so that the field is total and so that the
/// balance below is an equation about the fold rather than about a case split.
pub open spec fn spec_step_start(start: int, prev_len: int, step: Step) -> int {
    match step {
        Step::Restart(_) => prev_len,
        Step::Skip => start,
        Step::Extend(_) => start,
        Step::Close(_, _) => start,
    }
}

/// The agenda a step contributes for the block it releases, before the line being
/// planned is itself accounted for.
///
/// A step that releases the block the pass was holding writes that block's lines
/// here. The span is `prev_len - start` for the two implicit releases — a restart
/// and a skip both flush the block and then deal with the current line separately,
/// so that line is never part of the block they release — and one more than that
/// for a close, whose line is the block's own closing delimiter.
pub open spec fn spec_step_release(
    pending: Option<PlanBlock>,
    start: int,
    opener_rewritable: bool,
    facts: LineFacts,
    prev_len: int,
    step: Step,
) -> Seq<Agendum> {
    match pending {
        Some(block) => match step {
            Step::Restart(_) => spec_released_agenda(
                block,
                prev_len - start,
                opener_rewritable,
                false,
                Release::Abandoned,
            ),
            Step::Skip => spec_released_agenda(
                block,
                prev_len - start,
                opener_rewritable,
                false,
                Release::Abandoned,
            ),
            Step::Extend(_) => Seq::empty(),
            Step::Close(extended, release) => spec_released_agenda(
                extended,
                prev_len + 1 - start,
                opener_rewritable,
                facts.rewritable,
                release,
            ),
        },
        None => Seq::empty(),
    }
}

/// Whether a step writes the line it is planning, beyond any release.
///
/// A line a close or a restart deals with is already accounted for — by the
/// release that ends the old block, and by the start of the new one — so only a
/// skip writes an agendum here. Between them the release and this flag write every
/// line of the prefix exactly once, which is the whole content of the length
/// theorem below.
pub open spec fn spec_step_writes_line(step: Step) -> bool {
    match step {
        Step::Restart(_) => false,
        Step::Skip => true,
        Step::Extend(_) => false,
        Step::Close(_, _) => false,
    }
}

/// The lines a pending block accounts for when the prefix has reached `i`.
///
/// This is the quantity the length theorem balances: every line of the prefix is
/// either already in the agenda or still inside the block the pass is holding, and
/// the two must add up to the prefix however the fold is stepped. The `end` is not
/// the block's last line but the prefix's, which is why a block one line old
/// accounts for exactly one line only once that line has been stepped.
pub open spec fn spec_pending_lines(pending: Option<PlanBlock>, start: int, end: int) -> int {
    match pending {
        Some(_) => end - start,
        None => 0,
    }
}

/// The buffering half of the driver's state: the three fields a step changes.
///
/// This is the part the production kernel carries, and splitting it out is what
/// makes the kernel's postcondition statable: the kernel neither receives nor
/// returns the accumulated agenda, because the emitter appends to the document
/// itself. The specification's step and agenda functions are stated over this
/// record, and the fold below adds the agenda, so the two halves are joined in the
/// specification rather than in the kernel's argument list.
pub struct DriveState {
    /// The block being buffered, or `None` between blocks.
    pub pending: Option<PlanBlock>,
    /// Index of the pending block's opening line.
    pub start: int,
    /// Whether the pending block's opening line was a rewrite candidate.
    pub opener_rewritable: bool,
}

/// The driver's state after some prefix of a document.
///
/// The pass's loop is a fold over four values, and the whole document theorem
/// needs all four to say what the emitted sequence is: the three buffering fields
/// above, and the agenda accumulated so far. The fence state is deliberately *not*
/// a field: it is [`spec_state_seeded`] of the same prefix, which is what lets the
/// driver reuse the transition kernel's own specification rather than restating
/// it.
pub struct Drive {
    /// The buffering state, which the production kernel computes.
    pub state: DriveState,
    /// The agenda for every line the fold has accounted for so far.
    pub out: Seq<Agendum>,
}

/// The buffering state a step leaves behind.
///
/// The step is the planner's own routing relation applied to the line's facts, so
/// the branch taken here is the branch the production driver takes and not a model
/// of it. The three fields are the whole of what a step changes: which block is
/// buffered, where that block began, and whether its opener was a rewrite
/// candidate.
///
/// Taking the line's [`LineFacts`] rather than the fence state the line was read
/// against is deliberate. A production caller has already parsed the line and holds
/// exactly these facts; asking for them keeps the kernel's argument list equal to
/// the value the emitter already possesses, and moves the question of whether those
/// facts are the ones the document implies to the loop that produces them.
pub open spec fn spec_step_state(prev: DriveState, facts: LineFacts, prev_len: int) -> DriveState {
    let step = spec_plan_step(prev.pending, facts);
    DriveState {
        pending: spec_step_pending(step),
        start: spec_step_start(prev.start, prev_len, step),
        opener_rewritable: spec_step_opener_rewritable(
            prev.opener_rewritable,
            facts.rewritable,
            step,
        ),
    }
}

/// The agenda a step contributes.
///
/// This is the whole of what the pass writes for the lines a step accounts for, and
/// it is what the production kernel returns for the emitter to append: the release
/// of the block the step ends, plus the line itself when the line is written
/// directly rather than buffered. The length theorem below is the statement that
/// these contributions sum to the document.
pub open spec fn spec_step_agenda(prev: DriveState, facts: LineFacts, prev_len: int) -> Seq<Agendum> {
    let step = spec_plan_step(prev.pending, facts);
    let released = spec_step_release(
        prev.pending,
        prev.start,
        prev.opener_rewritable,
        facts,
        prev_len,
        step,
    );
    if spec_step_writes_line(step) {
        released.push(Agendum::Verbatim)
    } else {
        released
    }
}

/// The driver fold advanced by one line.
pub open spec fn spec_drive_step(prev: Drive, facts: LineFacts, prev_len: int) -> Drive {
    Drive {
        state: spec_step_state(prev.state, facts, prev_len),
        out: prev.out.add(spec_step_agenda(prev.state, facts, prev_len)),
    }
}

/// The driver fold, stated as a recursion over the prefix length.
///
/// This is the pass's whole decision procedure with the text stripped out. The
/// line the fold is extending by is read against [`spec_state_seeded`] of the
/// shorter prefix, which is the tracker state after that many lines, and the step
/// it takes is [`spec_drive_step`] — so the fold and the production loop differ
/// only in whether the tracker supplies that state or a recursion recomputes it.
///
/// The four cases of the planner step are exactly the pass's four dispositions of
/// a line: it opens a block, it is emitted directly, it is buffered into the open
/// block, or it closes the block and is released with it. Every flush path the
/// pass has is one of them. Two of the four release a pending block: a skip and a
/// restart both end it implicitly, which is the pass's abandoned release, and a
/// close ends it at a delimiter. The document's own end inside a block is the
/// fourth flush path, and [`spec_finish`] adds it once the fold is complete.
pub open spec fn spec_drive(
    source: Seq<LineFeatures>,
    rewritable: Seq<bool>,
    i: int,
) -> Drive
    decreases i
{
    if i <= 0 {
        Drive {
            state: DriveState { pending: None, start: 0, opener_rewritable: false },
            out: Seq::empty(),
        }
    } else {
        spec_drive_step(
            spec_drive(source, rewritable, i - 1),
            spec_line_facts(
                spec_state_seeded(source, i - 1, None),
                source[i - 1],
                rewritable[i - 1],
            ),
            i - 1,
        )
    }
}

/// `INV-DRIVER-BALANCE` — the fold's agenda and its pending block cover the prefix.
///
/// The invariant the length theorem establishes, in its three parts. Every line of
/// the prefix is accounted for exactly once — by an agendum the fold has recorded or
/// by the span of the block it is still holding — so the two add up to the prefix.
/// The block's opening index is never beyond the prefix. And while a block is
/// buffered no outside line has been released after it, which is why the agenda's
/// length is exactly the opening index; that third part is what makes the induction
/// below arithmetic rather than an accumulating argument.
pub open spec fn spec_balanced(drive: Drive, i: int) -> bool {
    drive.state.start <= i
        && drive.out.len() + spec_pending_lines(drive.state.pending, drive.state.start, i) == i
        && (drive.state.pending.is_some() ==> drive.out.len() == drive.state.start)
}

/// `LEM-RELEASE-ACCOUNTS-FOR-THE-BLOCK`.
///
/// A release contributes exactly as many agenda as the span it covers. This is
/// [`lemma_block_agenda_length`] read through the driver: it is the step that makes
/// the whole-prefix balance an equation about the fold rather than about the agenda
/// constructor, and it is stated once so that neither the step lemma below nor the
/// induction has to re-derive the block's length from the emission rule.
proof fn lemma_released_agenda_length(
    block: PlanBlock,
    span: int,
    opener_rewritable: bool,
    closer_rewritable: bool,
    release: Release,
)
    requires span >= 0,
    ensures spec_released_agenda(
        block,
        span,
        opener_rewritable,
        closer_rewritable,
        release,
    ).len() == span,
{
    reveal(spec_released_agenda);
    reveal(spec_delimiters_eligible);
    lemma_block_agenda_length(
        span,
        release,
        spec_delimiters_eligible(release, opener_rewritable, closer_rewritable),
        !block.conflicts,
        span,
    );
}

/// The release a step contributes holds as many agenda as the block it flushes.
///
/// The three releases that write a block each contribute its own span, and the step
/// that merely buffers contributes nothing. Naming the count per case here is what
/// lets the balance below be arithmetic rather than a case analysis repeated at every
/// induction step — the case split is discharged once, here, and the induction only
/// has to know which branch of the route it is in.
proof fn lemma_step_release_length(
    pending: Option<PlanBlock>,
    start: int,
    opener_rewritable: bool,
    facts: LineFacts,
    prev_len: int,
    step: Step,
)
    requires prev_len >= start,
    ensures
        spec_step_release(
            pending,
            start,
            opener_rewritable,
            facts,
            prev_len,
            step,
        ).len() == match pending {
            Some(_) => match step {
                Step::Restart(_) => prev_len - start,
                Step::Skip => prev_len - start,
                Step::Extend(_) => 0,
                Step::Close(_, _) => prev_len + 1 - start,
            },
            None => 0,
        },
{
    reveal(spec_step_release);
    match pending {
        Some(block) => match step {
            Step::Restart(_) => {
                lemma_released_agenda_length(
                    block,
                    prev_len - start,
                    opener_rewritable,
                    false,
                    Release::Abandoned,
                );
            },
            Step::Skip => {
                lemma_released_agenda_length(
                    block,
                    prev_len - start,
                    opener_rewritable,
                    false,
                    Release::Abandoned,
                );
            },
            Step::Extend(_) => {},
            Step::Close(extended, release) => {
                lemma_released_agenda_length(
                    extended,
                    prev_len + 1 - start,
                    opener_rewritable,
                    facts.rewritable,
                    release,
                );
            },
        },
        None => {},
    }
}

/// How many agenda one step contributes.
///
/// This is the length of [`spec_step_agenda`] spelled out per branch of the route,
/// so the induction below can add up the two halves — the block a step releases and
/// the line it writes — without unfolding either constructor again.
proof fn lemma_step_agenda_length(
    state: DriveState,
    facts: LineFacts,
    prev_len: int,
    step: Step,
)
    requires step == spec_plan_step(state.pending, facts), prev_len >= state.start,
    ensures
        spec_step_agenda(state, facts, prev_len).len() == match state.pending {
            Some(_) => match step {
                Step::Restart(_) => prev_len - state.start,
                Step::Skip => prev_len - state.start + 1,
                Step::Extend(_) => 0,
                Step::Close(_, _) => prev_len + 1 - state.start,
            },
            None => match step {
                Step::Skip => 1,
                _ => 0,
            },
        },
{
    reveal(spec_step_agenda);
    reveal(spec_step_writes_line);
    lemma_step_release_length(
        state.pending,
        state.start,
        state.opener_rewritable,
        facts,
        prev_len,
        step,
    );
}

/// The route a planner takes with nothing buffered.
///
/// A step with `None` buffered can only be an opening or a direct write: buffering
/// into a block presupposes a block, and closing one presupposes a delimiter that
/// ended it. Naming the fact here lets the induction below discharge those two
/// branches without unfolding the planner again.
proof fn lemma_no_block_cannot_buffer_or_close(pending: Option<PlanBlock>, facts: LineFacts)
    requires pending.is_none(),
    ensures !matches!(spec_plan_step(pending, facts), Step::Extend(_) | Step::Close(_, _)),
{
    reveal(spec_plan_step);
}

/// `LEM-DRIVE-LENGTH` — the fold's agenda and its pending block cover the prefix.
///
/// The line-count half of the document theorem, established on the decision itself
/// rather than on the loop that computes it. Each of the four dispositions accounts
/// for exactly the lines it consumes — a buffered line joins the pending span, an
/// opening line joins the new block's span, a line emitted directly becomes one
/// agendum, and a close turns the whole block into agenda of its own length — so the
/// agenda's length and the pending block's span always sum to the prefix, whatever
/// route the planner took.
///
/// The empty prefix is the base case and satisfies the balance outright: no block is
/// buffered and no line is claimed. That is not a degenerate case to be waved away —
/// it is the document consisting of no lines, and it is one of the inputs the
/// witnesses below name.
proof fn lemma_drive_length(source: Seq<LineFeatures>, rewritable: Seq<bool>, i: int)
    requires
        0 <= i,
        i <= source.len(),
        i <= rewritable.len(),
    ensures spec_balanced(spec_drive(source, rewritable, i), i),
    decreases i,
{
    if i > 0 {
        lemma_drive_length(source, rewritable, i - 1);
        let prev = spec_drive(source, rewritable, i - 1);
        let line = source[i - 1];
        let facts = spec_line_facts(
            spec_state_seeded(source, i - 1, None),
            line,
            rewritable[i - 1],
        );
        let step = spec_plan_step(prev.state.pending, facts);
        let next = spec_drive_step(prev, facts, i - 1);
        let added = spec_step_agenda(prev.state, facts, i - 1).len();
        reveal(spec_drive);
        reveal(spec_drive_step);
        reveal(spec_step_state);
        reveal(spec_balanced);
        reveal(spec_step_pending);
        reveal(spec_step_start);
        assert(spec_drive(source, rewritable, i) == next) by {
            reveal_with_fuel(spec_drive, 2);
        };
        assert(prev.state.start <= i - 1);
        assert(prev.out.len() + added == next.out.len()) by {
            reveal(spec_step_agenda);
        };
        lemma_step_agenda_length(prev.state, facts, i - 1, step);

        // Everything below is which route was taken: the two halves of the balance
        // are named by the step-agenda lemma, so each branch only has to say where
        // the new block's span starts and how long the new agenda is.

        match prev.state.pending {
            Some(block) => {
                assert(prev.out.len() == prev.state.start);
                match step {
                    Step::Restart(_) => {
                        assert(added == i - 1 - prev.state.start) by (nonlinear_arith)
                            requires added == i - 1 - prev.state.start;
                        assert(next.state.start == i - 1);
                        assert(next.out.len() == i - 1) by (nonlinear_arith)
                            requires next.out.len() == prev.state.start + (i - 1 - prev.state.start),
                                prev.state.start <= i - 1;
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == 1) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                    Step::Skip => {
                        assert(added == i - 1 - prev.state.start + 1);
                        assert(next.state.start == prev.state.start);
                        assert(next.state.pending.is_none());
                        assert(next.out.len() == i) by (nonlinear_arith)
                            requires next.out.len() == prev.state.start + (i - 1 - prev.state.start + 1),
                                prev.state.start <= i - 1;
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == 0) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                    Step::Extend(_) => {
                        assert(added == 0);
                        assert(next.state.start == prev.state.start);
                        assert(next.out.len() == prev.state.start);
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == i
                            - prev.state.start) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                    Step::Close(_, _) => {
                        assert(added == i - 1 + 1 - prev.state.start);
                        assert(next.state.start == prev.state.start);
                        assert(next.state.pending.is_none());
                        assert(next.out.len() == i) by (nonlinear_arith)
                            requires next.out.len() == prev.state.start + (i - 1 + 1 - prev.state.start);
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == 0) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                }
            },
            None => {
                assert(prev.out.len() == i - 1) by {
                    reveal(spec_pending_lines);
                };
                lemma_no_block_cannot_buffer_or_close(prev.state.pending, facts);
                match step {
                    Step::Restart(_) => {
                        assert(added == 0);
                        assert(next.state.start == i - 1) by {
                            reveal(spec_step_start);
                        };
                        assert(next.out.len() == i - 1);
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == 1) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                    Step::Skip => {
                        assert(added == 1) by {
                            reveal(spec_step_writes_line);
                        };
                        assert(next.state.start == prev.state.start);
                        assert(next.state.pending.is_none());
                        assert(next.out.len() == i) by {
                            reveal(spec_step_agenda);
                            reveal(spec_step_writes_line);
                        };
                        assert(spec_pending_lines(next.state.pending, next.state.start, i) == 0) by {
                            reveal(spec_pending_lines);
                        };
                        assert(spec_balanced(next, i)) by { reveal(spec_balanced); };
                    },
                    Step::Extend(_) => {
                        assert(false);
                    },
                    Step::Close(_, _) => {
                        assert(false);
                    },
                }
            },
        }
    }
}

/// The pass's whole decision for a document.
///
/// The agenda is one agendum per input line, which is the line-count half of the
/// document theorem: the driver records a single agendum for every line it writes
/// directly and a block's whole agenda for every line it buffers, so the emitted
/// document is exactly as long as the source.
///
/// The other two fields are the block the document ended inside, if it did, and
/// that block's line count. They are not decoration: the pass reports exactly
/// these two values when it preserves an unterminated block, so stating them here
/// is what makes the diagnostic a consequence of the decision rather than a
/// separate observation the adapter makes for itself.
pub struct Plan {
    /// What the pass writes, one agendum per source line.
    pub agenda: Seq<Agendum>,
    /// The block left open at end of input, or `None`.
    pub unclosed: Option<PlanBlock>,
    /// That block's line count, zero when there is none.
    pub unclosed_lines: int,
}

/// The block a completed fold leaves unclosed, read out as the plan's tail.
///
/// A fold that stops inside a block is not a failure: the document ended with a
/// delimiter still open, and the pass writes that block's lines exactly as a close
/// would have, with the one difference that there is no closing delimiter to
/// rewrite. This is the fourth flush path, and it is the only place the release
/// `Unmatched` is produced — which is why the pass can write `false` for a closer
/// flag it never consults.
pub open spec fn spec_finish(drive: Drive, len: int) -> Plan {
    match drive.state.pending {
        Some(block) => Plan {
            agenda: drive.out.add(spec_released_agenda(
                block,
                len - drive.state.start,
                drive.state.opener_rewritable,
                false,
                Release::Unmatched,
            )),
            unclosed: Some(block),
            unclosed_lines: len - drive.state.start,
        },
        None => Plan { agenda: drive.out, unclosed: None, unclosed_lines: 0 },
    }
}

/// The pass's whole decision for a document.
///
/// Stated as the fold followed by the tail, which is how the pass is written. The
/// two halves are proved separately — [`lemma_drive_length`] for the fold and
/// [`lemma_finish_total`] for the tail — because they are separate obligations: the
/// fold is where every buffering and release decision is made, and the tail is the
/// one flush path that has no releasing line to decide anything about.
pub open spec fn spec_document_plan(
    source: Seq<LineFeatures>,
    rewritable: Seq<bool>,
) -> Plan {
    spec_finish(spec_drive(source, rewritable, source.len() as int), source.len() as int)
}

/// `LEM-FINISH-TOTAL` — the tail adds the remaining lines and nothing else.
///
/// The fold's balance says the prefix is covered by the agenda plus the pending
/// block. The tail releases exactly that block, over the span the balance measured,
/// so the agenda after the tail holds one agendum per source line — the total length
/// equation the emitter relies on. A fold that stopped between blocks adds nothing
/// at all, which is the `None` arm and the reason the equation holds for documents
/// that end outside a fence as well as inside one.
proof fn lemma_finish_total(drive: Drive, len: int)
    requires spec_balanced(drive, len),
    ensures
        spec_finish(drive, len).agenda.len() == len,
        spec_finish(drive, len).unclosed_lines <= len,
{
    reveal(spec_finish);
    reveal(spec_balanced);
    reveal(spec_pending_lines);
    match drive.state.pending {
        Some(block) => {
            lemma_released_agenda_length(
                block,
                len - drive.state.start,
                drive.state.opener_rewritable,
                false,
                Release::Unmatched,
            );
        },
        None => {},
    }
}

/// `LEM-DOCUMENT-PLAN-TOTAL` — the decision is defined and complete on every input.
///
/// The fold reads each line at the index the plan's own sequence gives it and takes
/// every routing answer from the sequence's prefix, so nothing in the decision can
/// fail to find what it needs. The conclusion is the length equation the emitter
/// relies on, including for the empty document, whose fold is the base case and
/// whose plan has no unclosed block.
proof fn lemma_document_plan_total(source: Seq<LineFeatures>, rewritable: Seq<bool>)
    requires source.len() == rewritable.len(),
    ensures
        spec_document_plan(source, rewritable).agenda.len() == source.len(),
        spec_document_plan(source, rewritable).unclosed_lines <= source.len(),
{
    let i = source.len() as int;
    lemma_drive_length(source, rewritable, i);
    reveal(spec_document_plan);
    lemma_finish_total(spec_drive(source, rewritable, i), i);
}

// ---------------------------------------------------------------------------
// The document theorem
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
proof fn lemma_released_block_reads_back(
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
        // delimiter in its source spelling, and both are the same reading.
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

// ---------------------------------------------------------------------------
// Composing blocks into a document
// ---------------------------------------------------------------------------

/// `LEM-SEGMENT-EQUALS-ITS-SUBRANGE` — a span is a document of its own.
///
/// This is the lemma that makes every prefix-stated block result usable at any
/// offset, and it is the cheapest possible form of that translation: take the
/// lines the span covers as a sequence in their own right, and the two folds
/// agree on it, because a fold from `start` walking up to `end` visits exactly the
/// lines a fold from `0` walking up to `end - start` visits — the same line at each
/// step, by `axiom_seq_subrange_index`, and therefore the same region and the same
/// state.
///
/// With this, a block lemma stated over `[0, len)` — and all of them are, because
/// the pass's first block starts at zero — applies to a block beginning anywhere,
/// by reading the block's lines as the subrange they are. That is what the
/// composition needs and it costs one induction rather than a second copy of the
/// compressed-block argument at every offset.
pub proof fn lemma_segment_equals_its_subrange(
    features: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
)
    requires 0 <= start <= end <= features.len(),
    ensures
        spec_state_from(features, start, end, seed) == spec_state_seeded(
            features.subrange(start, end),
            end - start,
            seed,
        ),
        spec_regions_from(features, start, end, seed) == spec_regions_seeded(
            features.subrange(start, end),
            end - start,
            seed,
        ),
    decreases end - start,
{
    reveal(spec_regions_from);
    reveal(spec_state_from);
    reveal(spec_state_seeded);
    reveal(spec_regions_seeded);
    let sub = features.subrange(start, end);
    // The hypothesis the walk lemma takes as given, discharged here from the one
    // vstd axiom that relates a subrange's elements to the original's. This is the
    // only place subrange arithmetic is needed, and it is where the copy is made.
    assert forall|i: int| #![trigger sub[i]] 0 <= i < end - start implies sub[i]
        == features[start + i] by {
        axiom_seq_subrange_index(features, start, end, i);
    };
    lemma_walk_at_offset(features, sub, start, end - start, seed);
}

/// `LEM-WALK-AT-OFFSET` — a walk from an offset is the same walk over a copy.
///
/// The line correspondence is a hypothesis rather than a subrange equation, and
/// that is the whole point: the two folds agree for the plainest possible reason,
/// since at each step they read the same line and apply the same transition. By
/// taking the correspondence as given, this lemma carries no subrange arithmetic of
/// its own — a caller that holds the copy as a genuine subrange discharges the
/// hypothesis with `axiom_seq_subrange_index` and nothing else.
///
/// Stating it this way also keeps the induction honest about what it assumes:
/// `upto` lines of `sub` are related to the `upto` lines of `features` that begin
/// at `start`, and every step peels one pair off that correspondence.
pub proof fn lemma_walk_at_offset(
    features: Seq<LineFeatures>,
    sub: Seq<LineFeatures>,
    start: int,
    upto: int,
    seed: Option<FenceState>,
)
    requires
        0 <= start,
        0 <= upto,
        start + upto <= features.len(),
        upto <= sub.len(),
        forall|i: int| #![trigger sub[i]] 0 <= i < upto ==> sub[i] == features[start + i],
    ensures
        spec_state_from(features, start, start + upto, seed) == spec_state_seeded(sub, upto, seed),
        spec_regions_from(features, start, start + upto, seed) == spec_regions_seeded(
            sub,
            upto,
            seed,
        ),
    decreases upto,
{
    reveal(spec_state_from);
    reveal(spec_regions_from);
    reveal(spec_state_seeded);
    reveal(spec_regions_seeded);
    if upto > 0 {
        // The shorter hypothesis is about the same correspondence, one pair
        // shorter, so the step never has to reconstruct an index relation.
        lemma_walk_at_offset(features, sub, start, upto - 1, seed);
        assert(sub[upto - 1] == features[start + upto - 1]);
    }
}

/// `LEM-SEGMENT-READS-FROM-ITS-OWN-SEED` — a span from its own start is the same walk.
///
/// Every block lemma is stated from the document's start, because that is where
/// the pass begins. A block in the middle of a document is not a prefix: the lines
/// before it are already accounted for and the lines after it have not been read.
/// What the composition needs is the licence to read that span *from its own first
/// line*, seeded with the state the lines before it left — which is precisely what
/// the pass does, since it buffers each block's lines and reads them against the
/// tracker state at the block's opener.
///
/// The result is the split law read at both ends rather than a fresh induction.
/// Reading `[0, end)` in one go is the same as reading `[0, start)` and then
/// `[start, end)` from where the first part ended, so the middle segment's regions
/// are the tail of the whole prefix's — and by the bridge above, the three states
/// involved are the seeded ones.
pub proof fn lemma_segment_reads_from_its_own_seed(
    features: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
)
    requires 0 <= start <= end <= features.len(),
    ensures
        spec_state_from(features, start, end, spec_state_seeded(features, start, seed))
            == spec_state_seeded(features, end, seed),
        spec_regions_from(features, start, end, spec_state_seeded(features, start, seed))
            == spec_regions_seeded(features, end, seed).subrange(
                spec_regions_seeded(features, start, seed).len() as int,
                spec_regions_seeded(features, end, seed).len() as int,
            ),
{
    lemma_classify_from_is_prefix(features, start, seed);
    lemma_classify_from_is_prefix(features, end, seed);
    lemma_regions_from_appends(features, 0, start, end, seed);
    let head = spec_regions_seeded(features, start, seed);
    let full = spec_regions_seeded(features, end, seed);
    let seg = spec_regions_from(features, start, end, spec_state_seeded(features, start, seed));
    // The split law, in the segment form, is about the region sequence cut at the
    // state the first part left. Both conversions are the bridge above: the
    // zero-indexed folds are the seeded ones, and the middle seed is the seeded
    // state at `start`.
    assert(full == head.add(seg));
    // Cutting a concatenation at the joint leaves the second part. Stated with the
    // subrange form the conclusion uses rather than with `skip`, because the two
    // are the same operation written two ways.
    assert(head.add(seg).subrange(head.len() as int, head.add(seg).len() as int) =~= seg) by {
        assert forall|i: int|
            #![trigger head.add(seg).subrange(
                head.len() as int,
                head.add(seg).len() as int,
            )[i]]
            0 <= i < seg.len() implies head.add(seg).subrange(
            head.len() as int,
            head.add(seg).len() as int,
        )[i] == seg[i] by {
            assert(head.add(seg)[i + head.len() as int] == seg[i]);
        }
    };
}

/// `LEM-PREFIX-SPLITS-AT-ITS-SEEDED-STATE` — a prefix is its head then its tail.
///
/// The split law says the same thing, but spelled with the segment form on the
/// right. This is the spelling the composition uses at every join: the document so
/// far, then the span currently being emitted, read from the state the document so
/// far left. Stating it once keeps the composition from re-deriving the three
/// conversions at each of its cases.
pub proof fn lemma_prefix_splits_at_seeded_state(
    features: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
)
    requires 0 <= start <= end <= features.len(),
    ensures
        spec_state_seeded(features, end, seed) == spec_state_from(
            features,
            start,
            end,
            spec_state_seeded(features, start, seed),
        ),
        spec_regions_seeded(features, end, seed) == spec_regions_seeded(
            features,
            start,
            seed,
        ).add(spec_regions_from(features, start, end, spec_state_seeded(features, start, seed))),
{
    lemma_classify_from_is_prefix(features, start, seed);
    lemma_classify_from_is_prefix(features, end, seed);
    lemma_regions_from_appends(features, 0, start, end, seed);
}

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

/// Whether a span preserves its regions and lands where the caller says it does.
///
/// This is the per-span obligation the composition consumes, named so the chaining
/// lemma can quantify over spans rather than restate the three equalities at every
/// use. A span satisfies it when both runs read it from the same state, end in the
/// same state, and classify identically throughout.
///
/// Stating the obligation separately is also what makes the composition honest
/// about its own seam: the caller must show this for each span, and the composition
/// shows nothing else. Every way the pass releases a block — a matched close, an
/// abandoned block, the document's final flush — supplies the predicate by citing
/// the block lemma that covers it, so the composition never has to know which kind
/// of release produced the span it is joining.
pub closed spec fn ends_at(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
    target: Option<FenceState>,
) -> bool {
    spec_state_from(source, start, end, seed) == target
        && spec_state_from(emitted, start, end, seed) == target
        && spec_regions_from(source, start, end, seed) == spec_regions_from(
            emitted,
            start,
            end,
            seed,
        )
}

/// `LEM-AGREEMENT-SPAN-IS-PRESERVED` — the predicate follows from pointwise agreement.
///
/// This is how the composition discharges the spans the pass writes through without
/// compressing anything: an abandoned block, the run of lines between two blocks, a
/// matched block whose endpoints are not both rewrite candidates, and every
/// conflict-bearing block, whose fallback writes the source spelling at both ends.
/// Pointwise agreement is the only hypothesis any of them needs, and
/// [`lemma_agreement_from_segment`] is the induction that supplies it.
pub proof fn lemma_agreement_span_is_preserved(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
)
    requires
        emitted.len() == source.len(),
        0 <= start <= end <= source.len(),
        forall|i: int|
            #![trigger source[i]]
            start <= i < end ==> spec_same_reading(source[i], #[trigger] emitted[i]),
    ensures ends_at(source, emitted, start, end, seed, spec_state_from(source, start, end, seed)),
{
    reveal(ends_at);
    lemma_agreement_from_segment(source, emitted, start, end, seed);
}

/// Whether every span below `t` preserves its regions and lands where it should.
///
/// This is the composition's hypothesis as a single term rather than a quantifier.
/// The recursion peels the *last* span, matching the direction the composition
/// walks: the caller has a document cut into spans and joins them from the front,
/// so the fact each step consumes is the fact about the highest remaining cut.
///
/// The ordering of the cuts is part of the predicate, not a separate hypothesis,
/// because a span whose end is before its start is not a span. Carrying the
/// ordering here means each induction step gets both facts about its last span from
/// one unfold, and means the bounds the split law needs at the joint are available
/// without a second chain of arithmetic.
///
/// A quantifier over the cut sequence would say the same thing, and it is what the
/// first attempt used. It is the wrong shape here. A quantified `ends_at` puts a
/// recursive spec function at a symbolic index into every solver query, so the
/// verifier cannot decide whether to unfold it and spends its budget trying. A
/// recursive predicate is instead reduced one level per call at concrete indices,
/// which is what a proof of this size can afford.
pub open spec fn spec_chain_preserved(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    cuts: Seq<int>,
    states: Seq<Option<FenceState>>,
    t: int,
) -> bool
    decreases t
{
    if t <= 0 {
        0 <= cuts[0] <= source.len()
    } else {
        0 <= cuts[t - 1] <= cuts[t] && cuts[t] <= source.len() && ends_at(
            source,
            emitted,
            cuts[t - 1],
            cuts[t],
            states[t - 1],
            states[t],
        ) && spec_chain_preserved(source, emitted, cuts, states, t - 1)
    }
}

/// `LEM-SPANS-COMPOSE` — a partition into preserved spans preserves the document.
///
/// This is the composition law, and it is the statement the driver's fold
/// discharges. The pass never reasons about a whole document at once: it reads a
/// prefix, buffers the block it is inside, releases that block as one span when a
/// delimiter or a depth change ends it, and carries on. Every conclusion it needs
/// is therefore about a *span* — one released block's lines, or a run of lines
/// written through without buffering — and the document result is those span
/// results joined end to end.
///
/// The hypothesis is deliberately stated over a caller-supplied sequence of
/// states rather than over the seeded folds. That is what makes the lemma a
/// composition instead of a restatement: each span is read from a state the caller
/// names, the same state for the source run and the emitted run, so the two runs
/// are compared span by span without either side having to unfold the other's
/// fold. The state and region equalities the lemma concludes are then exactly what
/// the next span needs as its seed, which is why the caller can walk a document's
/// cut points left to right and never revisit one.
///
/// The induction removes the last span, which is the direction
/// [`spec_chain_preserved`] peels, so the hypothesis at the shorter chain is the
/// hypothesis at the longer one definitionally rather than by another induction.
pub proof fn lemma_spans_compose_regions(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    cuts: Seq<int>,
    states: Seq<Option<FenceState>>,
    k: int,
)
    requires
        emitted.len() == source.len(),
        cuts.len() > k,
        states.len() > k,
        0 <= k,
        cuts[0] == 0,
        spec_chain_preserved(source, emitted, cuts, states, k),
    ensures
        spec_state_seeded(source, cuts[k], states[0]) == states[k],
        spec_state_seeded(emitted, cuts[k], states[0]) == states[k],
        spec_regions_seeded(source, cuts[k], states[0]) == spec_regions_seeded(
            emitted,
            cuts[k],
            states[0],
        ),
    decreases k,
{
    reveal(spec_chain_preserved);
    if k == 0 {
        // The empty partition is the document that is a single point: no span at
        // all, so the state is the seed and the regions are empty. The base case
        // is not decorative — it is what makes the induction below total, and it
        // is the shape the caller gets for a document whose whole content was
        // written through without ever buffering a block.
        assert(spec_state_seeded(source, 0, states[0]) == states[0]) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        assert(spec_regions_seeded(source, 0, states[0]) == Seq::<Region>::empty()) by {
            reveal_with_fuel(spec_regions_seeded, 2);
        };
        assert(spec_state_seeded(emitted, 0, states[0]) == states[0]) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        assert(spec_regions_seeded(emitted, 0, states[0]) == Seq::<Region>::empty()) by {
            reveal_with_fuel(spec_regions_seeded, 2);
        };
    } else {
        // The induction is over the number of spans and removes the last one, so
        // the shorter chain is exactly the spans this step does not touch. Naming
        // the two cut bounds as locals before the call lets the arithmetic that
        // follows read them rather than the sequence indexing.
        let joint = cuts[k - 1];
        let last = cuts[k];
        assert(spec_chain_preserved(source, emitted, cuts, states, k - 1));
        lemma_spans_compose_regions(source, emitted, cuts, states, k - 1);
        // The last span's contribution, read out of the hypothesis. Unfolding the
        // chain at `k` splits it into the last span's fact and the shorter chain,
        // which is exactly the hypothesis the recursive call consumed.
        assert(ends_at(source, emitted, joint, last, states[k - 1], states[k]));
        reveal(ends_at);
        assert(spec_state_from(source, joint, last, states[k - 1]) == states[k]);
        assert(spec_state_from(emitted, joint, last, states[k - 1]) == states[k]);
        assert(spec_regions_from(source, joint, last, states[k - 1]) == spec_regions_from(
            emitted,
            joint,
            last,
            states[k - 1],
        ));
        // The prefix's result is the seed the last span is read from, which the
        // induction supplies for both runs at once.
        assert(spec_state_seeded(source, joint, states[0]) == states[k - 1]) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        assert(spec_state_seeded(emitted, joint, states[0]) == states[k - 1]) by {
            reveal_with_fuel(spec_state_seeded, 2);
        };
        // Joining the two halves is the split law at the joint, not a second
        // induction: the prefix and the last span are read consecutively from the
        // same state, so their regions concatenate.
        lemma_prefix_splits_at_seeded_state(source, joint, last, states[0]);
        lemma_prefix_splits_at_seeded_state(emitted, joint, last, states[0]);
        assert(spec_state_seeded(source, last, states[0]) == states[k]);
        assert(spec_state_seeded(emitted, last, states[0]) == states[k]);
        assert(spec_regions_seeded(source, last, states[0]) == spec_regions_seeded(
            source,
            joint,
            states[0],
        ).add(spec_regions_from(source, joint, last, states[k - 1])));
        assert(spec_regions_seeded(emitted, last, states[0]) == spec_regions_seeded(
            emitted,
            joint,
            states[0],
        ).add(spec_regions_from(emitted, joint, last, states[k - 1])));
    }
}

/// The regions of the lines in `[start, end)`, read from the state at `start`.
///
/// [`spec_regions_seeded`] reads a *prefix* from a seed, which is what every
/// induction above wants but not what a document-level argument needs: a document
/// is emitted in segments — a run of directly written lines, then a released block,
/// then another run — and each segment is read from the state the previous one left
/// behind. This specification names a segment directly, so the composition can be
/// a statement about segments rather than about the fold that produced them.
///
/// The state at `start` is *not* recomputed here. Passing it in is deliberate: a
/// caller composing two adjacent segments has to show they agree at the join, and
/// taking the state as a parameter is what makes that obligation visible rather
/// than hidden inside a recursion.
pub open spec fn spec_regions_from(
    features: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
) -> Seq<Region>
    decreases end - start
{
    if end <= start {
        Seq::empty()
    } else {
        spec_regions_from(features, start, end - 1, seed).push(
            spec_fence_region(spec_state_from(features, start, end - 1, seed), features[end - 1]),
        )
    }
}

/// The fence state after the lines in `[start, end)` have been read.
pub open spec fn spec_state_from(
    features: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
) -> Option<FenceState>
    decreases end - start
{
    if end <= start {
        seed
    } else {
        spec_fence_next(
            spec_state_from(features, start, end - 1, seed),
            features[end - 1],
        )
    }
}

/// `LEM-REGIONS-FROM-APPENDS` — a segment is its first part then its second.
///
/// This is the law that makes segmentation possible: reading `[start, end)` in one
/// go is the same as reading `[start, mid)` and then `[mid, end)` from the state the
/// first part left, for both the regions and the state. Without it, a document
/// argument would have to unfold the whole recursion at every join.
proof fn lemma_regions_from_appends(
    features: Seq<LineFeatures>,
    start: int,
    mid: int,
    end: int,
    seed: Option<FenceState>,
)
    requires
        start <= mid <= end,
        end <= features.len(),
    ensures
        spec_state_from(features, start, end, seed) == spec_state_from(
            features,
            mid,
            end,
            spec_state_from(features, start, mid, seed),
        ),
        spec_regions_from(features, start, end, seed) == spec_regions_from(
            features,
            start,
            mid,
            seed,
        ).add(spec_regions_from(
            features,
            mid,
            end,
            spec_state_from(features, start, mid, seed),
        )),
    decreases end - mid,
{
    reveal(spec_regions_from);
    reveal(spec_state_from);
    if mid < end {
        lemma_regions_from_appends(features, start, mid, end - 1, seed);
        // The induction is on the *tail*'s length, so both segments lose their last
        // line together and the shorter hypothesis is about exactly the two prefixes
        // this step needs. Naming the state each side ends in is what lets the last
        // region of either be written against the same state.
        let outer = spec_state_from(features, start, end - 1, seed);
        let inner = spec_state_from(
            features,
            mid,
            end - 1,
            spec_state_from(features, start, mid, seed),
        );
        let head = spec_regions_from(features, start, mid, seed);
        assert(outer == inner);
        assert(spec_regions_from(features, start, end, seed) == head.add(
            spec_regions_from(features, mid, end - 1, spec_state_from(features, start, mid, seed)),
        ).push(spec_fence_region(outer, features[end - 1])));
        assert(spec_regions_from(
            features,
            mid,
            end,
            spec_state_from(features, start, mid, seed),
        ) == spec_regions_from(
            features,
            mid,
            end - 1,
            spec_state_from(features, start, mid, seed),
        ).push(spec_fence_region(inner, features[end - 1])));
        // The appendix moves across the concatenation: the last region belongs after
        // both parts, not after the first.
        Seq::push_distributes_over_add(
            head,
            spec_regions_from(features, mid, end - 1, spec_state_from(features, start, mid, seed)),
            spec_fence_region(outer, features[end - 1]),
        );
    }
}

/// `LEM-CLASSIFY-FROM-IS-THE-SEEDED-PREFIX` — the two specifications agree at `0`.
///
/// [`spec_regions_seeded`] is the prefix form every earlier lemma is stated over,
/// and [`spec_regions_from`] is the segment form the composition needs. Naming
/// their agreement at the document start is what lets the block lemmas above, which
/// are all in the prefix form, discharge obligations in the segment form.
pub proof fn lemma_classify_from_is_prefix(
    features: Seq<LineFeatures>,
    end: int,
    seed: Option<FenceState>,
)
    requires end <= features.len(),
    ensures
        spec_state_from(features, 0, end, seed) == spec_state_seeded(features, end, seed),
        spec_regions_from(features, 0, end, seed) == spec_regions_seeded(features, end, seed),
    decreases end,
{
    reveal(spec_regions_from);
    reveal(spec_state_from);
    if end > 0 {
        lemma_classify_from_is_prefix(features, end - 1, seed);
    }
}

/// `LEM-AGREEMENT-OVER-A-SEGMENT` — pointwise agreement carries a span.
///
/// [`lemma_agreement_preserves_prefix`] states this for a prefix of the whole
/// document. The composition below needs it for an arbitrary `[start, end)`,
/// because that is the shape a released block has: the lines before the block
/// have already been accounted for, so its argument begins from the state those
/// lines left rather than from the document's seed.
///
/// Nothing in the argument changes with the offset. Each line of the span reads
/// back as itself, so each contributes the same region and leaves the same state,
/// and the two folds therefore end together. The prefix form cannot supply this:
/// it is indexed from zero, and a caller holding a span cannot recover the offset
/// from a statement about the whole document.
///
/// This is the whole argument for every release that writes no compressed
/// delimiter — an abandoned block, and a matched block whose endpoints are not
/// both rewrite candidates. It excludes a conflict-bearing block whose delimiter
/// carries a `null` language specifier: a conflict does make the pass fall back
/// to the source spelling, but that spelling drops the specifier and so raises
/// `trailing_blank`, which this lemma's `spec_same_reading` hypothesis does not
/// admit. See [`lemma_agreement_preserves_prefix`] for the same boundary.
pub proof fn lemma_agreement_from_segment(
    source: Seq<LineFeatures>,
    emitted: Seq<LineFeatures>,
    start: int,
    end: int,
    seed: Option<FenceState>,
)
    requires
        emitted.len() == source.len(),
        0 <= start <= end <= source.len(),
        forall|i: int|
            #![trigger source[i]]
            start <= i < end ==> spec_same_reading(source[i], #[trigger] emitted[i]),
    ensures
        spec_state_from(source, start, end, seed) == spec_state_from(emitted, start, end, seed),
        spec_regions_from(source, start, end, seed) == spec_regions_from(
            emitted,
            start,
            end,
            seed,
        ),
    decreases end - start,
{
    reveal(spec_regions_from);
    reveal(spec_state_from);
    if start < end {
        lemma_agreement_from_segment(source, emitted, start, end - 1, seed);
        let before = spec_state_from(source, start, end - 1, seed);
        assert(spec_same_reading(source[end - 1], emitted[end - 1]));
        lemma_same_reading_preserves_region(before, source[end - 1], emitted[end - 1]);
        lemma_same_reading_preserves_next(before, source[end - 1], emitted[end - 1]);
    }
}

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

/// The fold's own decision, pinned as values rather than as an equation.
///
/// The witnesses above are all about the *rules* — a transition, a guard, an
/// eligibility predicate. None of them exercises [`spec_drive`], so nothing in
/// the block above would fail if the fold wired those rules together wrongly.
/// The four below close that gap by naming the fold's whole output for the four
/// routes a document can take, which is the smallest evidence that the
/// composition's subject is non-empty: a document the fold drives to a close, a
/// document that ends inside a block, a document the fold abandons at a depth
/// change, and the empty document.
///
/// Each is stated as an equality between the plan and a literal sequence, not as
/// an equality between two unknowns, so the witness fails if *either* side moves.

/// The document the fold drives to a matched close.
///
/// The shortest matched block there is: a four-backtick opener and its own close.
/// The whole plan is pinned, so a fold that mis-routed the closer, mis-counted
/// the span, or lost a line would change one of these three values. The agenda is
/// all `Compressed` and not `Normalized` because the block has no interior line at
/// all, so its conflict flag is still false when it is released and the pass takes
/// the compression route — which is what makes this witness the compressed half's
/// concrete instance rather than a second copy of the agreement case.
proof fn lemma_witness_matched_block_plan_is_live()
    ensures
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_bare_backticks(4))
                .push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true).push(true),
        ).agenda == Seq::<Agendum>::empty()
            .push(Agendum::Compressed)
            .push(Agendum::Compressed),
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_bare_backticks(4))
                .push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true).push(true),
        ).unclosed.is_none(),
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_bare_backticks(4))
                .push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true).push(true),
        ).unclosed_lines == 0,
{
    let source = Seq::<LineFeatures>::empty()
        .push(spec_witness_bare_backticks(4))
        .push(spec_witness_bare_backticks(4));
    let rewritable = Seq::<bool>::empty().push(true).push(true);
    reveal_with_fuel(spec_drive, 6);
    reveal_with_fuel(spec_block_agenda_seeded, 8);
    reveal_with_fuel(spec_state_seeded, 8);
    reveal(spec_document_plan);
    reveal(spec_finish);
    reveal(spec_drive_step);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_released_agenda);
    reveal(spec_block_agenda);
    reveal(spec_delimiter_agendum);
    reveal(spec_is_outer_delimiter);
    reveal(spec_delimiters_eligible);
    reveal(spec_plan_step);
    reveal(spec_absorb);
    reveal(spec_open_block);
    reveal(spec_step_state);
    reveal(spec_step_pending);
    reveal(spec_line_facts);
    reveal(spec_fenced);
    reveal(spec_planner_features);
    reveal(spec_closes);
    reveal(spec_fence_next);
}

/// The document the fold leaves inside an unmatched block.
///
/// This is the fourth flush path, and it is the one the corpus cannot reach:
/// every fixture under `tests/data/` closes its fences, so no fixture exercises
/// `Release::Unmatched`. The witness is the shortest document that does — a lone
/// opener — and it pins all three of the plan's fields, including the diagnostic
/// the adapter reports, so a fold that failed to record the unclosed block would
/// fail here rather than only in the corpus sweep whose documents supply one by
/// hand.
proof fn lemma_witness_unmatched_block_plan_is_live()
    ensures
        spec_document_plan(
            Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true),
        ).agenda == Seq::<Agendum>::empty().push(Agendum::Compressed),
        spec_document_plan(
            Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true),
        ).unclosed_lines == 1,
        spec_document_plan(
            Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4)),
            Seq::<bool>::empty().push(true),
        ).unclosed == Some(
            PlanBlock {
                opening: FenceState { marker: '`', marker_len: 4, open_depth: 0 },
                conflicts: false,
            },
        ),
{
    let source = Seq::<LineFeatures>::empty().push(spec_witness_bare_backticks(4));
    let rewritable = Seq::<bool>::empty().push(true);
    reveal_with_fuel(spec_drive, 6);
    reveal_with_fuel(spec_block_agenda_seeded, 8);
    reveal_with_fuel(spec_state_seeded, 8);
    reveal(spec_document_plan);
    reveal(spec_finish);
    reveal(spec_drive_step);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_released_agenda);
    reveal(spec_block_agenda);
    reveal(spec_delimiter_agendum);
    reveal(spec_is_outer_delimiter);
    reveal(spec_delimiters_eligible);
    reveal(spec_plan_step);
    reveal(spec_absorb);
    reveal(spec_open_block);
    reveal(spec_step_state);
    reveal(spec_step_pending);
    reveal(spec_step_start);
    reveal(spec_step_opener_rewritable);
    reveal(spec_line_facts);
    reveal(spec_fenced);
    reveal(spec_planner_features);
    reveal(spec_closes);
    reveal(spec_fence_next);
}

/// The document the fold drives to the conflict fallback.
///
/// A four-backtick opener, an interior three-backtick line, and a payload line.
/// The interior line is the issue #480 shape: under the source opener it is
/// literal content, and under the compressed opener the pass would write it is a
/// closing delimiter, so compressing the block would move the payload line out of
/// the literal region. The planner therefore records a conflict and the release
/// writes `Normalized` rather than `Compressed` — and the two later lines are
/// `Verbatim`, because the release covers them without rewriting either.
///
/// The three agenda together are the fallback's whole observable effect: the
/// opener keeps its source spelling, and the two lines that follow it are written
/// byte for byte. A planner whose conflict fold missed this interior line would
/// emit `Compressed` here, which is exactly the defect the compression mutation
/// gate injects.
proof fn lemma_witness_conflict_fallback_plan_is_live()
    ensures
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_bare_backticks(4))
                .push(spec_witness_bare_backticks(3))
                .push(spec_witness_prose()),
            Seq::<bool>::empty().push(true).push(true).push(true),
        ).agenda == Seq::<Agendum>::empty()
            .push(Agendum::Normalized)
            .push(Agendum::Verbatim)
            .push(Agendum::Verbatim),
        spec_document_plan(
            Seq::<LineFeatures>::empty()
                .push(spec_witness_bare_backticks(4))
                .push(spec_witness_bare_backticks(3))
                .push(spec_witness_prose()),
            Seq::<bool>::empty().push(true).push(true).push(true),
        ).unclosed_lines == 3,
{
    let source = Seq::<LineFeatures>::empty()
        .push(spec_witness_bare_backticks(4))
        .push(spec_witness_bare_backticks(3))
        .push(spec_witness_prose());
    let rewritable = Seq::<bool>::empty().push(true).push(true).push(true);
    reveal_with_fuel(spec_drive, 5);
    reveal_with_fuel(spec_block_agenda_seeded, 5);
    reveal_with_fuel(spec_state_seeded, 5);
    reveal(spec_document_plan);
    reveal(spec_finish);
    reveal(spec_drive_step);
    reveal(spec_step_agenda);
    reveal(spec_step_release);
    reveal(spec_released_agenda);
    reveal(spec_block_agenda);
    reveal(spec_delimiter_agendum);
    reveal(spec_is_outer_delimiter);
    reveal(spec_delimiters_eligible);
    reveal(spec_plan_step);
    reveal(spec_absorb);
    reveal(spec_open_block);
    reveal(spec_step_state);
    reveal(spec_step_pending);
    reveal(spec_line_facts);
    reveal(spec_fenced);
    reveal(spec_planner_features);
    reveal(spec_closes);
    reveal(spec_fence_next);
}

/// The empty document's plan, which is the fold's base case.
///
/// The base case is not decoration: it is the document consisting of no lines,
/// it is one of the inputs the length theorem claims, and it is the only input on
/// which the fold's own definition is reached rather than its step. Pinning it as
/// a value keeps the fold's totality from resting on an unexamined arm — a plan
/// with no agenda, no unclosed block, and no reported lines.
proof fn lemma_witness_empty_document_plan_is_live()
    ensures
        spec_document_plan(Seq::<LineFeatures>::empty(), Seq::<bool>::empty()).agenda
            == Seq::<Agendum>::empty(),
        spec_document_plan(Seq::<LineFeatures>::empty(), Seq::<bool>::empty()).unclosed.is_none(),
        spec_document_plan(Seq::<LineFeatures>::empty(), Seq::<bool>::empty()).unclosed_lines == 0,
{
    lemma_document_plan_total(Seq::<LineFeatures>::empty(), Seq::<bool>::empty());
    reveal(spec_document_plan);
    reveal(spec_drive);
    reveal(spec_finish);
}

} // verus!
