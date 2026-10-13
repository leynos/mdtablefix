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
//! change any field the transition reads, so a normalized delimiter is read back
//! as a delimiter with the same marker, the same run, the same depth, and the
//! same blank-tag decision. Region equality then follows from a *pointwise
//! reading agreement*, stated once in [`lemma_agreement_preserves_prefix`] and
//! applied to every line the planner leaves alone. That is why the planner needs
//! only an eligibility predicate, and why those cases need no transition
//! reasoning at all.
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

/// The raw-text contract the document theorem is proved from.
///
/// The pass does not reason about text; it renders one of three agenda into
/// bytes, and a separate regular expression reads those bytes back on the next
/// pass. Verus cannot and should not reason about a compiled regular expression,
/// so the relationship between the two is an external contract, recorded here as
/// a predicate and in the verification ledger as an assumed boundary.
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
        // language specifier dropped. A null specifier contributes nothing the
        // transition reads: both the blank-tag decision and the run length are
        // decided by the marker characters and by the remaining text, neither of
        // which dropping it can change.
        Agendum::Normalized => line.normalizable ==> spec_line_agrees(line),
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

/// `LEM-DOCUMENT-REGIONS-PRESERVED-BY-AGREEMENT`.
///
/// If every line of a prefix reads back as the line it was written from, the
/// prefix classifies identically in both runs and leaves the same fence state
/// behind. The result holds for every prefix, which is what a document-level
/// induction over the pass needs, and it needs no hypothesis about blocks,
/// releases, or markers: agreement is enough on its own.
///
/// This covers every document the pass writes without a compressed delimiter —
/// including every abandoned block, every matched block whose endpoints are not
/// both rewrite candidates, and every conflict-bearing block, because a conflict
/// makes the pass write its delimiters in their source spelling.
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
/// regions identical, because a normalized delimiter reads back as the very
/// delimiter it was written from.
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
    reveal_with_fuel(spec_state_seeded, 4);
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

} // verus!
