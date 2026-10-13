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
    plan::{self, Agendum, DriverState, LineFacts, Release},
};
use crate::wrap::FenceTracker;

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

/// The lines the pass is holding, in document order.
///
/// The planner's buffering state lives in [`DriverState`] and is never stored
/// here; this is only the text, kept rendered rather than borrowed because a
/// block outlives the borrow of each source line.
type PendingLines = Vec<CachedLine>;

/// Append the agenda the driver returned to the document.
///
/// The buffer and the agenda are two renderings of the same lines, and this is
/// where they meet: the planner said what to write for each of them, and the
/// cached line says what those words spell. Nothing here decides anything — no
/// branch reads a flag or a length to choose a spelling — which is what the
/// proof requires of the emitter and what the pass now cannot violate.
///
/// The buffer is drained from the front because the agenda covers its leading
/// entries: [`plan::drive_step`] returns one agendum per line the step accounted
/// for, so the caller's obligations are to have buffered those lines first and
/// to append this agenda once.
fn drain_agenda(lines: &mut PendingLines, agenda: Vec<Agendum>, out: &mut Vec<String>) {
    debug_assert!(
        agenda.len() <= lines.len(),
        "the planner's agenda cannot name more lines than the pass has buffered",
    );
    out.extend(
        lines
            .drain(..agenda.len().min(lines.len()))
            .zip(agenda)
            .map(|(line, agendum)| line.emitted(agendum)),
    );
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

/// Emit the block a document ended inside, if it did.
///
/// The document's own end is the fourth flush path: the driver's last call was a
/// buffer, so the lines it accounted for are still in the pass's hands and the
/// planner has not written them. The tail is the release `Unmatched`, the one
/// release no line produces, and it is named here rather than by the driver
/// because the driver has no line left to plan.
///
/// The diagnostic is the decision read out, not a second opinion about it: the
/// two reported values are the buffered line count and the conflict flag the
/// planner accumulated, both taken from what the driver returned.
fn flush_unclosed(state: DriverState, lines: &mut PendingLines, out: &mut Vec<String>) {
    let Some(block) = state.pending else {
        return;
    };
    debug!(
        block_lines = lines.len(),
        has_conflicting_interior_fence = block.conflicts,
        "preserving an unclosed fence block"
    );
    drain_agenda(
        lines,
        plan::released_agenda(
            block,
            lines.len(),
            state.opener_rewritable,
            false,
            Release::Unmatched,
        ),
        out,
    );
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
    let mut state = DriverState::new();
    let mut buffered: PendingLines = Vec::new();
    let mut out = Vec::with_capacity(lines.len());

    for (index, line) in lines.iter().enumerate() {
        // Parse each source line once: the tracker supplies the blockquote depth
        // and structural fence marker, and the planner decides from those facts
        // what the pass does with the line and with the block it is holding.
        let parsed = ParsedLine::observe(&mut tracker, line);
        let facts = parsed.facts;
        // The line joins the buffer before the step is taken, because the driver
        // returns an agenda for the buffer as it then stands: a step that ends the
        // block writes lines the pass has already appended, and a step that opens
        // one leaves the new line as the sole entry the next step can reach.
        buffered.push(parsed.into_cached());
        let (next, agenda) = plan::drive_step(state, facts, index);
        state = next;
        // Whatever the step did not account for stays buffered. The agenda names
        // the leading entries and no others, so draining by its length leaves
        // exactly the block the driver is still holding.
        drain_agenda(&mut buffered, agenda, &mut out);
    }

    // A block the document ended inside has no closing delimiter to rewrite.
    flush_unclosed(state, &mut buffered, &mut out);

    out
}
