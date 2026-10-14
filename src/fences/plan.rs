//! Pure compression planner behind the module's `compress_fences` pass.
//!
//! The planner answers one question for every line of the input: does the pass
//! write a delimiter in place of this line, and if so which spelling? It owns
//! the pass's buffering state — the opening delimiter and the accumulated
//! conflict flag — decides which positions of a released block are rewritten,
//! and produces the agenda the emitter writes from.
//!
//! Everything here is a pure function of values the caller has already parsed:
//! no regex, no buffering of text, and no I/O. `verus/lib.rs` includes this
//! production body directly, so the proofs in `verus/plan_spec.rs` constrain
//! the decisions the formatter actually makes rather than a parallel model free
//! to drift.
//!
//! Two questions are deliberately not answered here. Whether a line is a
//! delimiter, and whether a delimiter can be respelled at all, both need the
//! delimiter grammar, which lives with the regexes the enclosing module is
//! built around; the caller answers them and passes the answers in, as line
//! features and as a rewrite-candidate flag. Keeping the grammar out is what
//! lets the planner stay regex-free, and therefore provable. The seam between
//! the two is an external contract recorded in `docs/verification.md`.

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

include!("../verified_kernel_macros.rs");

// The kernels this planner is proved against are included by the proof entry
// point under a `production_fence` module rather than as part of the library,
// so the two builds reach the same types by different paths. Naming both keeps
// the body identical in each, which is the point of including this file rather
// than reimplementing it.
#[cfg(verus_keep_ghost)]
use crate::production_fence::{
    FenceState as KernelState,
    LineFeatures,
    compression_changes_region,
};
#[cfg(not(verus_keep_ghost))]
use crate::wrap::{KernelState, LineFeatures, compression_changes_region};

/// The planner's driver loop: the buffering state, and the agenda a step
/// releases.
///
/// Extracted so that neither file grows past the repository's 400-line
/// ceiling (`AGENTS.md`). Both builds reach it through this declaration, so
/// the library's child module and `verus/lib.rs`'s `production_plan` hold the
/// same source.
#[path = "plan_driver.rs"]
mod driver;
pub use driver::{DriverState, drive_step, released_agenda};

/// The buffering state of the block the pass currently holds open.
///
/// A block is described by the opener it was created from and by the answer to
/// its one whole-block question: has any interior line's region become
/// rewrite-sensitive? The line count is not stored, because the caller already
/// holds the buffered lines and its length is the only source of truth.
#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Block {
    /// Opening-fence state, the left half of every rewrite decision.
    pub opening: KernelState,
    /// Whether an interior line makes compression change the block's meaning.
    pub conflicts: bool,
}

/// Why a buffered block stopped accumulating lines.
///
/// The release fixes which of the block's positions may be rewritten: a matched
/// block has two delimiters in play, an unterminated one has only its opening
/// delimiter, and an abandoned block has none, because the lines it buffered
/// were never bounded by a delimiter the pass read.
#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum Release {
    /// The block closed at its opening depth.
    Matched,
    /// The document, or the enclosing blockquote, ended inside the block.
    Unmatched,
    /// A line the tracker reads outside the block arrived while the block was
    /// still open, so the block ends implicitly with no closing delimiter.
    Abandoned,
}

/// What the pass writes in place of one source line.
///
/// The variants are the whole vocabulary of the agenda, and they are named for
/// the bytes they produce rather than for a state, because the emitter — not
/// the planner — turns them into text.
#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum Agendum {
    /// Write the source line unchanged.
    Verbatim,
    /// Write the line's normalization: the source marker family and run, with a
    /// null language specifier dropped.
    Normalized,
    /// Write the compressed delimiter: three backticks, whatever the source used.
    Compressed,
}

/// What the caller learned about one source line before planning it.
///
/// The two booleans are the tracker's own routing answers, and the features and
/// rewrite flag are the two grammar-dependent facts the planner is not allowed
/// to derive for itself. Nothing here is recomputed: every field is a value the
/// caller already holds, so planning cannot read a line differently from the
/// transition that classified it.
#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct LineFacts {
    /// Whether the tracker was inside a fence at this line's depth before it
    /// stepped the line.
    pub was_in_fence: bool,
    /// Whether the tracker is inside a fence at this line's depth afterwards.
    pub is_in_fence: bool,
    /// The kernel's view of the line, absent when the line is not fence-shaped.
    ///
    /// Absence is exactly the tracker's "not a fence marker" answer, so the
    /// planner derives its own fencing test from this one value.
    pub features: Option<LineFeatures>,
    /// Whether the line carries a delimiter rewrite the emitter could write.
    pub rewritable: bool,
}

/// What the caller must do with one line, and with the block it was holding.
///
/// A variant that carries a [`Block`] carries the *updated* buffering state, so
/// the caller never applies a step by hand: it stores what it is given. That is
/// what keeps the planner, and not the driver, the authority on when a block is
/// open and whether it may be compressed.
#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum Step {
    /// Emit any buffered block unchanged, then buffer this line as a new
    /// block's opening delimiter.
    Restart(Block),
    /// Emit any buffered block unchanged, then emit this line unchanged.
    Skip,
    /// Buffer this line into the open block, which stays open.
    Extend(Block),
    /// Buffer this line into the open block, then emit the block with this
    /// release.
    Close(Block, Release),
}

verified_kernel_function! {
/// The block a fence delimiter opens.
///
/// The opening state comes from the line's own marker, run length, and depth:
/// the tracker has already read this line as opening a block, so its features
/// describe the opener exactly. `None` is the caller's answer that the line is
/// not fence-shaped, which is not an error but a line the pass emits directly.
#[must_use]
pub fn open_block(line: LineFacts) -> Option<Block>;
ensures(result => result@ == crate::spec_open_block(line@));
{
    let features = line.features?;
    let marker = features.marker?;
    Some(Block {
        opening: KernelState {
            marker,
            marker_len: features.marker_len,
            open_depth: features.depth,
        },
        conflicts: false,
    })
}
}

verified_kernel_function! {
/// Accumulate one buffered line into a block's conflict flag.
///
/// A block is compressed only when no interior line's region could change, and
/// this is the running answer to that question. `block.conflicts` carries the
/// answer so far, which is why this is a left fold rather than a test of the
/// line alone: once any interior line conflicts, the block keeps its source
/// delimiters and every later line's answer is irrelevant.
///
/// A line the tracker read as not fence-shaped contributes nothing, and neither
/// does a closing delimiter, so the caller can absorb every buffered line
/// without deciding which of them could conflict.
#[must_use]
pub fn absorb(block: Block, line: Option<LineFeatures>) -> Block;
ensures(result => result@ == crate::spec_absorb(block@, line@));
{
    Block {
        opening: block.opening,
        conflicts: block.conflicts || match line {
            Some(features) => compression_changes_region(block.opening, features),
            None => false,
        },
    }
}
}

verified_kernel_function! {
/// Decide what one line does to the block the pass is holding.
///
/// The two routing answers the tracker supplies are exhaustive and disjoint
/// here. A line outside the fence at its own depth either opens a block or is
/// emitted directly; a line inside it is buffered into the open block, and the
/// block closes exactly when the line is a fence marker the tracker no longer
/// reads as inside a fence, because a line that leaves the fence open at its
/// own depth is a delimiter that did not close anything and is therefore
/// literal interior content.
#[must_use]
pub fn plan_step(block: Option<Block>, line: LineFacts) -> Step;
ensures(result => result == crate::spec_plan_step(block@, line@));
{
    if line.was_in_fence {
        match block {
            Some(open) => {
                let extended = absorb(open, line.features);
                if line.features.is_some() && !line.is_in_fence {
                    Step::Close(extended, Release::Matched)
                } else {
                    Step::Extend(extended)
                }
            }
            None => Step::Skip,
        }
    } else {
        match open_block(line) {
            Some(fresh) => Step::Restart(fresh),
            None => Step::Skip,
        }
    }
}
}

verified_kernel_function! {
/// The tracker's own answer to whether a fence is in force at `depth`.
///
/// This is the single routing test the planner is driven by, stated as a
/// function of the tracker's state so that the driver's two answers and the
/// driver's own bookkeeping cannot be derived twice and disagree. A line at or
/// below the open fence's depth is inside it; a line shallower than that is not,
/// whatever state the tracker carries, because a shallower line ends the fence
/// before it is read.
#[must_use]
pub fn fenced(state: Option<KernelState>, depth: usize) -> bool;
ensures(result => result == crate::spec_fenced(state@, depth));
{
    matches!(state, Some(open) if depth >= open.open_depth)
}
}

verified_kernel_function! {
/// Whether a release may respell the delimiters it writes.
///
/// The answer is the caller's recognition that every delimiter the release puts
/// in play is a rewrite candidate: the opener alone for an unterminated block,
/// both ends for a matched one, and nothing at all for an abandoned block. When
/// either required end is rejected the whole block keeps its source spelling,
/// which is what preserves the opener's recognition.
#[must_use]
pub fn delimiters_eligible(
    release: Release,
    opener_rewritable: bool,
    closer_rewritable: bool,
) -> bool;
ensures(result => result == crate::spec_delimiters_eligible(
    release,
    opener_rewritable,
    closer_rewritable,
));
{
    match release {
        Release::Abandoned => false,
        Release::Unmatched => opener_rewritable,
        Release::Matched => opener_rewritable && closer_rewritable,
    }
}
}

verified_kernel_function! {
/// Whether position `index` of a released block of `len` lines is one of its
/// delimiters.
///
/// The opener is always a delimiter; the last line is one only for a matched
/// block, because an unterminated block has no closing delimiter to write. The
/// block's length is a parameter rather than a precomputed last index so the
/// predicate needs no partial subtraction, and so a caller cannot pair a
/// position with the last index of a different block.
#[must_use]
pub fn is_outer_delimiter(index: usize, len: usize, release: Release, written: bool) -> bool;
ensures(result => result == crate::spec_is_outer_delimiter(
    index as int,
    len as int,
    release,
    written,
));
{
    written
        && (index == 0
            || (matches!(release, Release::Matched) && len > 0 && index == len - 1))
}
}

verified_kernel_function! {
/// What the pass writes for one position of a released block.
///
/// A position the planner does not write reaches the document as it was
/// written, which is what keeps every interior line's bytes — and therefore its
/// region — intact. A delimiter the pass does write takes the compressed
/// spelling when the block has no conflicting interior line, and the source
/// spelling otherwise: the fallback is what keeps a conflict-bearing block's
/// regions identical, because a normalized delimiter establishes the very state
/// the source delimiter established.
#[must_use]
pub fn delimiter_agendum(is_delimiter: bool, compressible: bool) -> Agendum;
ensures(result => result == crate::spec_delimiter_agendum(is_delimiter, compressible));
{
    if !is_delimiter {
        Agendum::Verbatim
    } else if compressible {
        Agendum::Compressed
    } else {
        Agendum::Normalized
    }
}
}

verified_loop_function! {
/// The agenda for a released block of `len` lines, position by position.
///
/// This is the whole emission decision for every flush path: which positions
/// are delimiters follows from the release, and what those delimiters are
/// spelled with follows from the block's one compression answer. Emitting a
/// block is therefore not three behaviours but one, and the driver's only
/// remaining choice is which release to name.
#[must_use]
pub fn block_agenda(
    len: usize,
    release: Release,
    written: bool,
    compressible: bool,
) -> Vec<Agendum>;
ensures(result => result@ == crate::spec_block_agenda(
    len as int,
    release,
    written,
    compressible,
));
before {
    let mut out: Vec<Agendum> = Vec::new();
    let mut index = 0;
}
while (index < len) invariant(
    index <= len,
    out@ == crate::spec_block_agenda_seeded(
        len as int,
        release,
        written,
        compressible,
        index as int,
    ),
) {
    let is_delimiter = is_outer_delimiter(index, len, release, written);
    out.push(delimiter_agendum(is_delimiter, compressible));
    index += 1;
}
after { out }
}

