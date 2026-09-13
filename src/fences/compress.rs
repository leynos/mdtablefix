//! Fence delimiter compression: the algorithm behind [`compress_fences`].
//!
//! The module parses each source line once through the structural fence tracker
//! and accumulates an opened block until it closes at its opening depth. A
//! matched block has its outer delimiters rewritten to three backticks unless an
//! interior fence-like line would become structural as a result; a block the
//! document ends inside rewrites only its opening delimiter.
//!
//! The parent's private `FENCE_RE` and `is_null_lang` are used from here because
//! orphan-specifier attachment needs them too; a child module reads them without
//! any visibility change. The parent re-exports the entry point as
//! [`crate::fences::compress_fences`].

use tracing::debug;

use super::{FENCE_RE, is_null_lang};
use crate::wrap::{FenceObservation, FenceTracker, ObservedFence};

/// Selects how a recognised fence marker is normalised.
#[derive(Clone, Copy)]
pub(super) enum Strategy {
    /// Compress compatible opening and closing markers to three backticks.
    Compress,
    /// Retain the source marker when interior content could conflict with compression.
    Preserve,
}

/// A retained source line together with its compressed rewrite, computed once
/// when the line was parsed.
///
/// Caching `compressed` avoids repeated compression work and supports
/// `flush_unmatched_block`, which rewrites only the opening delimiter.
struct CachedLine {
    /// Original source bytes retained for interior content and fallback emission.
    line: String,
    /// The line rewritten with a compressed three-backtick delimiter, or `None`
    /// when the line is not a normalization-compatible fence delimiter.
    compressed: Option<String>,
}
/// Buffers one candidate fenced block until its closing marker determines the rewrite.
struct PendingFenceBlock {
    /// Marker family and length from the opening delimiter.
    opening_marker: String,
    /// Whether an interior marker makes compression change the block's meaning.
    has_conflicting_interior_fence: bool,
    /// Source lines and any precomputed compatible rewrites in document order.
    lines: Vec<CachedLine>,
}

/// Returns the marker family used by a fence, if the delimiter is non-empty.
fn marker_char(marker: &str) -> Option<char> { marker.chars().next() }

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

/// Reports whether an interior marker would conflict with the opening delimiter after compression.
fn interior_fence_requires_preserved_delimiters(
    opening_marker: &str,
    parsed: Option<(&str, &str, &str)>,
) -> bool {
    let Some((_indent, marker, _info)) = parsed else {
        return false;
    };
    let Some(opening_ch) = marker_char(opening_marker) else {
        return false;
    };
    let Some(marker_ch) = marker_char(marker) else {
        return false;
    };
    marker_ch == opening_ch || marker_ch == '`'
}

/// Chooses preservation whenever interior fence-like content would make compression ambiguous.
fn opening_rewrite(has_conflicting_interior_fence: bool) -> Strategy {
    if has_conflicting_interior_fence {
        Strategy::Preserve
    } else {
        Strategy::Compress
    }
}

/// Emit a fence line, reusing its cached compressed rewrite for the
/// `Compress` strategy and computing the preserved rewrite on demand.
///
/// The `Preserve` strategy is only chosen once per block, so its
/// rewrite is not worth caching per line.
fn rewrite_fence_line(cached: CachedLine, strategy: Strategy) -> String {
    let CachedLine { line, compressed } = cached;
    match strategy {
        Strategy::Compress => compressed.unwrap_or(line),
        Strategy::Preserve => rewrite_marker(&line, Strategy::Preserve).unwrap_or(line),
    }
}

/// Emits an unmatched block, normalising only its opening marker and preserving its body.
fn flush_unmatched_block(block: PendingFenceBlock, out: &mut Vec<String>) {
    debug!(
        block_lines = block.lines.len(),
        has_conflicting_interior_fence = block.has_conflicting_interior_fence,
        "preserving an unclosed fence block"
    );
    // The block never closed, so its interior lines are literal content of the
    // unclosed fence: rewrite only the opening delimiter and emit every
    // interior line verbatim, so fence-like content is not rewritten. The
    // opening delimiter takes the same rewrite as the matched path, because
    // compressing an opener that contains a conflicting interior fence is
    // self-defeating here too: it shortens, or changes the family of, the
    // delimiter that made the interior line literal, and the next pass reads
    // that line as the closer instead.
    let rewrite = opening_rewrite(block.has_conflicting_interior_fence);

    for (index, cached) in block.lines.into_iter().enumerate() {
        let emitted = if index == 0 {
            rewrite_fence_line(cached, rewrite)
        } else {
            cached.line
        };
        out.push(emitted);
    }
}

/// Emits a matched block, rewriting both delimiters when the interior is safe.
fn flush_matched_block(block: PendingFenceBlock, out: &mut Vec<String>) {
    let rewrite = opening_rewrite(block.has_conflicting_interior_fence);
    let closing_index = block.lines.len() - 1;
    for (index, cached) in block.lines.into_iter().enumerate() {
        let emitted = if index == 0 || index == closing_index {
            rewrite_fence_line(cached, rewrite)
        } else {
            cached.line
        };
        out.push(emitted);
    }
}

/// Emits every line from a block exactly as it appeared in the source.
fn flush_original_block(block: PendingFenceBlock, out: &mut Vec<String>) {
    out.extend(block.lines.into_iter().map(|cached| cached.line));
}

/// Emit a completed block, rewriting its delimiters when both ends carry a
/// cached compressed rewrite and otherwise preserving the original lines.
fn flush_completed_block(block: PendingFenceBlock, out: &mut Vec<String>) {
    let opening_rewritable = block.lines.first().is_some_and(|c| c.compressed.is_some());
    let closing_rewritable = block.lines.last().is_some_and(|c| c.compressed.is_some());
    if opening_rewritable && closing_rewritable {
        flush_matched_block(block, out);
    } else {
        flush_original_block(block, out);
    }
}

/// A source line parsed once for `compress_fences`.
///
/// Bundles the fence-state observation, the structural marker components, and
/// the compressed rewrite so that opening, closing, conflicting-interior, and
/// flush decisions all draw from a single parse of the line.
struct ParsedLine<'a> {
    /// Original source line borrowed until the block is cached.
    line: &'a str,
    /// Structural fence state produced by the shared tracker.
    observation: FenceObservation,
    /// Marker components parsed by the tracker, if this line is a fence.
    fence: Option<(&'a str, &'a str, &'a str)>,
    /// Optional three-backtick rewrite computed from the same source parse.
    compressed: Option<String>,
}

impl<'a> ParsedLine<'a> {
    /// Observe `line` against `tracker` and compute its compressed rewrite once.
    ///
    /// The blockquote depth and structural fence marker come from the tracker's
    /// single parse via [`FenceTracker::observe_source_fence`]; only the local
    /// normalization regex runs in addition, so the raw line is never handed to
    /// `is_fence` again.
    fn observe(tracker: &mut FenceTracker, line: &'a str) -> Self {
        let observed: ObservedFence<'a> = tracker.observe_source_fence(line);
        Self {
            line,
            observation: observed.observation,
            fence: observed.fence,
            compressed: rewrite_marker(line, Strategy::Compress),
        }
    }

    /// Owns the borrowed source line for storage in a pending block.
    fn into_cached(self) -> CachedLine {
        CachedLine {
            line: self.line.to_owned(),
            compressed: self.compressed,
        }
    }
}
/// Begin a pending fence block for a line observed outside any active fence.
///
/// Any block that was still pending is emitted verbatim first. When the line
/// opens a fence a fresh block is returned; otherwise the (possibly compressed)
/// line is pushed to `out` and `None` is returned.
fn start_fence_block(
    previous: Option<PendingFenceBlock>,
    parsed: ParsedLine<'_>,
    out: &mut Vec<String>,
) -> Option<PendingFenceBlock> {
    if let Some(block) = previous {
        flush_original_block(block, out);
    }
    let Some((_indent, opening_marker, _info)) = parsed.fence else {
        out.push(parsed.compressed.unwrap_or_else(|| parsed.line.to_owned()));
        return None;
    };
    let opening_marker = opening_marker.to_owned();
    Some(PendingFenceBlock {
        opening_marker,
        has_conflicting_interior_fence: false,
        lines: vec![parsed.into_cached()],
    })
}

/// Advance the pending fence block for a line observed inside an active fence,
/// returning the block that remains pending afterwards (if any).
///
/// Interior fence markers are accumulated as literal content until the block
/// closes at its opening depth, at which point it is flushed.
fn advance_fence_block(
    pending: Option<PendingFenceBlock>,
    parsed: ParsedLine<'_>,
    out: &mut Vec<String>,
) -> Option<PendingFenceBlock> {
    let Some(mut block) = pending else {
        out.push(parsed.line.to_owned());
        return None;
    };

    let observation = parsed.observation;
    if observation.is_fence_marker
        && observation.is_in_fence
        && interior_fence_requires_preserved_delimiters(&block.opening_marker, parsed.fence)
    {
        block.has_conflicting_interior_fence = true;
    }

    let keep_open = !observation.is_fence_marker || observation.is_in_fence;
    block.lines.push(parsed.into_cached());

    if keep_open {
        return Some(block);
    }

    flush_completed_block(block, out);
    None
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
/// When input ends inside an unclosed fence, `compress_fences` uses
/// `flush_unmatched_block`, which rewrites only the opening delimiter, under
/// the same preserved-delimiter rule, and emits every interior line verbatim,
/// so fence-like content inside that unclosed block is preserved rather than
/// rewritten.
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
        // and structural fence marker, and the compressed rewrite is computed a
        // single time and cached on the block for every flush path.
        let parsed = ParsedLine::observe(&mut tracker, line);
        pending_block = if parsed.observation.was_in_fence {
            advance_fence_block(pending_block.take(), parsed, &mut out)
        } else {
            start_fence_block(pending_block.take(), parsed, &mut out)
        };
    }

    if let Some(block) = pending_block {
        flush_unmatched_block(block, &mut out);
    }

    out
}

