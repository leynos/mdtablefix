//! Fence delimiter compression: the algorithm behind [`compress_fences`].
//!
//! This module is the pass's adapter, not its decision-maker. Every choice it
//! makes — whether a block is open, whether the block may be compressed, which
//! positions of a released block are delimiters, and what spelling those
//! delimiters take — comes from [`plan`], whose production body `verus/lib.rs`
//! proves against `verus/plan_spec.rs`. What remains here is the part a proof
//! cannot reach: parsing each source line through the shared [`FenceTracker`],
//! spelling the two delimiter rewrites the planner's agenda selects, and
//! buffering rendered lines until the planner releases the block.
//!
//! The parent's private `FENCE_RE` and `is_null_lang` are used from here because
//! orphan-specifier attachment needs them too; a child module reads them without
//! any visibility change. The parent re-exports the entry point as
//! [`crate::fences::compress_fences`].

use tracing::debug;

use super::{
    FENCE_RE,
    is_null_lang,
    plan::{self, Agendum, LineFacts, Release, Step},
};
use crate::wrap::FenceTracker;

/// The planner state of the block the pass is holding, if any.
type PendingState = Option<plan::Block>;

/// Selects how a recognised fence marker is normalised.
///
/// The two spellings are the whole of what the emitter adds to the planner's
/// agenda: the planner says `Compressed` or `Normalized`, and this type says
/// what characters those two names are written with.
#[derive(Clone, Copy)]
pub(super) enum Strategy {
    /// Compress compatible opening and closing markers to three backticks.
    Compress,
    /// Retain the source marker when interior content could conflict with compression.
    Preserve,
}

/// A retained source line together with both delimiter respellings, computed
/// once when the line was parsed.
///
/// Caching both rewrites keeps the emitter a lookup rather than a second parse,
/// so no line can be spelled one way while the planner's agenda was built from
/// another reading of it.
struct CachedLine {
    /// Original source bytes, written for every position the planner leaves alone.
    source: String,
    /// The line with its source marker family and run, and a null language
    /// specifier dropped, or `None` when the line is not a delimiter the local
    /// grammar recognises.
    normalized: Option<String>,
    /// The line rewritten with a compressed three-backtick delimiter, or `None`
    /// for the same reason as `normalized`.
    compressed: Option<String>,
}

impl CachedLine {
    /// The bytes the planner's agenda writes for this position.
    fn emitted(&self, agendum: Agendum) -> String {
        match agendum {
            Agendum::Verbatim => self.source.clone(),
            // A delimiter the planner chose not to compress is written with its
            // own marker; a line the rewrite cannot spell falls back to its
            // source bytes rather than to nothing.
            Agendum::Normalized => self
                .normalized
                .clone()
                .unwrap_or_else(|| self.source.clone()),
            Agendum::Compressed => self
                .compressed
                .clone()
                .unwrap_or_else(|| self.source.clone()),
        }
    }
}

/// One pending block: the planner's buffering state plus the lines it holds.
///
/// The planner owns `block`; this struct only stores what it is handed. The
/// lines are kept rendered rather than borrowed because a block outlives the
/// borrow of each source line.
struct PendingBlock {
    /// Opening-fence state and accumulated conflict flag, owned by the planner.
    block: plan::Block,
    /// Source lines and their cached rewrites, in document order.
    lines: Vec<CachedLine>,
}

/// Rewrites a parsed fence marker according to the selected compression strategy.
///
/// Null language specifiers are omitted so a normalised fence does not acquire a literal null tag.
pub(super) fn rewrite_marker(line: &str, strategy: Strategy) -> Option<String> {
    let cap = FENCE_RE.captures(line)?;
    let indent = cap.get(1).map_or("", |m| m.as_str());
    let original_marker = cap.get(2).map_or("", |m| m.as_str());
    let lang = cap.get(3).map_or("", |m| m.as_str());
    let marker = match strategy {
        Strategy::Compress => "```",
        Strategy::Preserve => original_marker,
    };
    Some(if is_null_lang(lang) {
        format!("{indent}{marker}")
    } else {
        format!("{indent}{marker}{lang}")
    })
}

/// A source line parsed once for `compress_fences`.
///
/// The tracker's single parse yields the blockquote depth, the fence state
/// transition, and the kernel's view of the line; the two respellings are
/// computed from the same line here. Nothing downstream re-reads the source, so
/// the planner's decision, the transition that produced it, and the bytes
/// written for it cannot disagree about what the line is.
struct ParsedLine<'a> {
    /// Original source line borrowed until the block is cached.
    line: &'a str,
    /// The tracker's routing answers and the kernel's view of this line.
    facts: LineFacts,
    /// Source spelling of the delimiter, or `None` when the line has none.
    normalized: Option<String>,
    /// Compressed spelling of the delimiter, or `None` when the line has none.
    compressed: Option<String>,
}

impl<'a> ParsedLine<'a> {
    /// Observe `line` against `tracker` and compute both respellings once.
    ///
    /// The blockquote depth, structural fence marker, and kernel features all
    /// come from the tracker's single parse via
    /// [`FenceTracker::observe_source_fence`]; only the local normalization
    /// regex runs in addition. Deriving the features from the same parse keeps
    /// the planner's decision and the fence transition from disagreeing about
    /// what the line is.
    fn observe(tracker: &mut FenceTracker, line: &'a str) -> Self {
        let observed = tracker.observe_source_fence(line);
        let compressed = rewrite_marker(line, Strategy::Compress);
        Self {
            line,
            facts: LineFacts {
                was_in_fence: observed.observation.was_in_fence,
                is_in_fence: observed.observation.is_in_fence,
                features: observed.features,
                rewritable: compressed.is_some(),
            },
            normalized: rewrite_marker(line, Strategy::Preserve),
            compressed,
        }
    }

    /// Owns the borrowed source line and its rewrites for storage in a block.
    fn into_cached(self) -> CachedLine {
        CachedLine {
            source: self.line.to_owned(),
            normalized: self.normalized,
            compressed: self.compressed,
        }
    }
}

/// Whether the block's opening line carries a rewrite the emitter could write.
fn opener_rewritable(lines: &[CachedLine]) -> bool {
    lines.first().is_some_and(|line| line.compressed.is_some())
}

/// Whether the block's last line carries a rewrite the emitter could write.
fn closer_rewritable(lines: &[CachedLine]) -> bool {
    lines.last().is_some_and(|line| line.compressed.is_some())
}

/// Emit a released block by looking up the planner's agenda for every position.
///
/// This is the whole of the pass's emission for all three releases. The planner
/// fixes which positions are delimiters, whether either is loose enough to
/// respell, and what the block's one compression answer is; this function only
/// writes the bytes those answers name, and emits the lines it did not rewrite
/// from their source, so interior payload cannot change.
fn flush_block(pending: PendingBlock, release: Release, out: &mut Vec<String>) {
    let PendingBlock { block, lines } = pending;
    let len = lines.len();
    if release == Release::Unmatched {
        debug!(
            block_lines = len,
            has_conflicting_interior_fence = block.conflicts,
            "preserving an unclosed fence block"
        );
    }

    let written = plan::delimiters_eligible(
        release,
        opener_rewritable(&lines),
        closer_rewritable(&lines),
    );
    let agenda = plan::block_agenda(len, release, written, !block.conflicts);
    out.extend(
        lines
            .into_iter()
            .zip(agenda)
            .map(|(line, agendum)| line.emitted(agendum)),
    );
}

/// Emit a block left pending when a later line arrived outside it.
///
/// A block ends this way when the blockquote depth drops below its opener, or
/// when the document's fence state was already lost. The lines it buffered were
/// never bounded by a closing delimiter the pass read, so the planner names the
/// release and writes all of them as they were written.
fn flush_abandoned(pending: Option<PendingBlock>, out: &mut Vec<String>) {
    if let Some(block) = pending {
        flush_block(block, Release::Abandoned, out);
    }
}

/// Apply one planner step to the pending block and the output.
///
/// Every branch that ends a block names a release and hands the block to
/// [`flush_block`]; none of them inspects a line or a flag to decide what to
/// write, so the pass cannot spell a delimiter the planner did not choose.
fn apply_step(
    pending: &mut Option<PendingBlock>,
    step: Step,
    parsed: ParsedLine<'_>,
    out: &mut Vec<String>,
) {
    match step {
        Step::Restart(block) => {
            flush_abandoned(pending.take(), out);
            *pending = Some(PendingBlock {
                block,
                lines: vec![parsed.into_cached()],
            });
        }
        Step::Skip => {
            flush_abandoned(pending.take(), out);
            // The planner opened nothing here and has nothing to buffer, so the
            // line reaches the document as written.
            out.push(parsed.line.to_owned());
        }
        Step::Extend(block) => {
            let Some(mut open) = pending.take() else {
                out.push(parsed.line.to_owned());
                return;
            };
            open.block = block;
            open.lines.push(parsed.into_cached());
            *pending = Some(open);
        }
        Step::Close(block, release) => {
            let Some(mut open) = pending.take() else {
                out.push(parsed.line.to_owned());
                return;
            };
            open.block = block;
            // The closing line is part of the block the planner just wrote: its
            // own answer is what released them together.
            open.lines.push(parsed.into_cached());
            flush_block(open, release, out);
        }
    }
}

/// Normalize safe outer fence delimiters to exactly three backticks.
///
/// `compress_fences` returns non-fence lines unchanged. Compatible backtick or
/// tilde delimiters in matched fenced blocks may be rewritten to three
/// backticks when doing so preserves the document structure.
/// Fence-like lines inside a wider matched fenced block are literal content and
/// are returned unchanged. An outer delimiter is also preserved when
/// shortening or changing it would make an inner literal fence line look
/// structural.
///
/// When input ends inside an unclosed fence, `compress_fences` rewrites only
/// the opening delimiter, under the same preserved-delimiter rule, and emits
/// every interior line verbatim, so fence-like content inside that unclosed
/// block is preserved rather than rewritten.
///
/// # Examples
///
/// ```
/// use mdtablefix::fences::compress_fences;
/// let out = compress_fences(&["````rust".to_string()]);
/// assert_eq!(out, vec!["```rust".to_string()]);
/// ```
#[must_use]
pub fn compress_fences(lines: &[String]) -> Vec<String> {
    let mut tracker = FenceTracker::new();
    let mut pending_block = None;
    let mut out = Vec::with_capacity(lines.len());

    for line in lines {
        // Parse each source line once: the tracker supplies the blockquote depth
        // and structural fence marker, and the planner decides from those facts
        // what the pass does with the line and with the block it is holding.
        let parsed = ParsedLine::observe(&mut tracker, line);
        let held: PendingState = pending_block.as_ref().map(|open: &PendingBlock| open.block);
        let step = plan::plan_step(held, parsed.facts);
        apply_step(&mut pending_block, step, parsed, &mut out);
    }

    // A block the document ended inside has no closing delimiter to rewrite.
    if let Some(block) = pending_block {
        flush_block(block, Release::Unmatched, &mut out);
    }

    out
}
