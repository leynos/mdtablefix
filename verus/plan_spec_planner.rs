//! The compression planner: one line in, one decision out.
//!
//! The planner is stated as a pure function of a line's features and the
//! block it is buffering. [`spec_plan_step`] is the routing decision,
//! [`spec_absorb_all`] the conflict fold over a block's interior,
//! [`spec_delimiter_agendum`] the emission decision for one delimiter, and
//! [`spec_fenced`] the tracker's own in-fence test, which is where the
//! planner meets the transition kernel.
//!
//! Extracted from `verus/plan_spec.rs` at a declaration seam so both files come
//! in under the 400-line ceiling `AGENTS.md` sets. [`lemma_block_agenda_length`]
//! stays `pub(super)` and is re-imported by the parent, because the driver's
//! length theorem — which lives in its own module — must still name it.

use crate::fence_spec::{spec_compression_changes_region, spec_fence_next};
use crate::production_fence::{FenceState, LineFeatures};
use crate::production_plan::{Agendum, Block as PlanBlock, LineFacts, Release, Step};
use vstd::prelude::*;

verus! {

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
pub(super) proof fn lemma_block_agenda_length(
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
pub(super) proof fn lemma_absorb_all_preserves_opening(
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
pub(super) proof fn lemma_no_conflict_all_clear(
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

} // verus!
