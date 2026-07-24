//! Pre-processing utilities for normalizing fenced code block delimiters.
//!
//! `compress_fences` reduces safe outer delimiters to three backticks while
//! preserving nested fence-like content whose marker runs are literal text.
//! The local `FENCE_RE` defines which delimiter lines this module can
//! normalize, while `FenceTracker` provides the depth-aware structural Markdown
//! fence semantics shared with wrapping; its `observe_source_fence` supplies the
//! structural marker parse so this module never re-runs `wrap::is_fence`.
//! `attach_orphan_specifiers` then finds orphaned fence specifier lines and
//! attaches them to the following fence, preserving the retained indentation
//! and normalized language specifier.
use std::sync::LazyLock;

use regex::Regex;

use crate::wrap::{FenceObservation, FenceTracker, ObservedFence};

mod attachment;

use attachment::attach_to_next_fence;

static FENCE_RE: LazyLock<Regex> = lazy_regex!(
    r"^(\s*)(`{3,}|~{3,})([A-Za-z0-9_+.,-]*)\s*$",
    "fence delimiter and language specifier pattern should compile",
);

static ORPHAN_LANG_RE: LazyLock<Regex> = lazy_regex!(
    r"^[A-Za-z0-9_+.-]*[A-Za-z0-9_+\-](?:,[A-Za-z0-9_+.-]*[A-Za-z0-9_+\-])*$",
    "orphaned fence language specifier pattern should compile",
);

/// Determine whether a language specifier denotes an absent language.
///
/// A language is absent when it is empty or the case-insensitive string `null`, with surrounding
/// whitespace ignored.
///
/// # Examples
///
/// ```rust,ignore
/// use mdtablefix::fences::is_null_lang;
/// assert!(is_null_lang(""));
/// assert!(is_null_lang("NULL"));
/// assert!(is_null_lang("  null  "));
/// assert!(!is_null_lang("rust"));
/// ```
#[inline]
fn is_null_lang(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null")
}

/// Normalize a potential language specifier.
///
/// Returns the cleaned specifier in lowercase and the leading indentation
/// captured from the original line.
///
/// # Examples
///
/// ```rust,ignore
/// use mdtablefix::fences::normalize_specifier;
/// let (spec, indent) = normalize_specifier("  TOML, Ini");
/// assert_eq!(spec, "toml,ini");
/// assert_eq!(indent, "  ");
/// ```
fn normalize_specifier(line: &str) -> (String, String) {
    let indent = crate::textproc::leading_indent(line).to_string();
    let cleaned = line
        .trim()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(",")
        .to_lowercase();
    (cleaned, indent)
}

#[derive(Clone, Copy)]
enum FenceRewrite {
    Compress,
    PreserveDelimiters,
}

#[derive(Clone, Copy)]
enum MarkerStrategy {
    Compressed,
    PreserveDelimiter,
}

/// A retained source line together with its compressed rewrite, computed once
/// when the line was parsed.
///
/// Caching `compressed` here lets every flush path emit the line without running
/// the normalization regex again, including `flush_unmatched_block`, which may
/// rewrite any retained line.
struct CachedLine {
    line: String,
    /// The line rewritten with a compressed three-backtick delimiter, or `None`
    /// when the line is not a normalization-compatible fence delimiter.
    compressed: Option<String>,
}
struct PendingFenceBlock {
    opening_marker: String,
    has_conflicting_interior_fence: bool,
    lines: Vec<CachedLine>,
}

fn marker_char(marker: &str) -> Option<char> { marker.chars().next() }

fn rewrite_marker(line: &str, strategy: MarkerStrategy) -> Option<String> {
    let cap = FENCE_RE.captures(line)?;
    let indent = cap.get(1).map_or("", |m| m.as_str());
    let original_marker = cap.get(2).map_or("", |m| m.as_str());
    let lang = cap.get(3).map_or("", |m| m.as_str());
    let marker = match strategy {
        MarkerStrategy::Compressed => "```",
        MarkerStrategy::PreserveDelimiter => original_marker,
    };
    Some(if is_null_lang(lang) {
        format!("{indent}{marker}")
    } else {
        format!("{indent}{marker}{lang}")
    })
}

fn compressed_fence_line(line: &str) -> Option<String> {
    rewrite_marker(line, MarkerStrategy::Compressed)
}

fn preserved_fence_line(line: &str) -> Option<String> {
    rewrite_marker(line, MarkerStrategy::PreserveDelimiter)
}

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

fn opening_rewrite(has_conflicting_interior_fence: bool) -> FenceRewrite {
    if has_conflicting_interior_fence {
        FenceRewrite::PreserveDelimiters
    } else {
        FenceRewrite::Compress
    }
}

/// Emit a delimiter line, reusing its cached compressed rewrite for the
/// `Compress` strategy and computing the preserved rewrite on demand.
///
/// The `PreserveDelimiters` strategy is only chosen once per block, so its
/// rewrite is not worth caching per line.
fn rewrite_delimiter(cached: CachedLine, rewrite: FenceRewrite) -> String {
    let CachedLine { line, compressed } = cached;
    match rewrite {
        FenceRewrite::Compress => compressed.unwrap_or(line),
        FenceRewrite::PreserveDelimiters => preserved_fence_line(&line).unwrap_or(line),
    }
}
fn flush_unmatched_block(block: PendingFenceBlock, out: &mut Vec<String>) {
    // The block never closed, so its interior lines are literal content of the
    // unclosed fence: normalize only the opening delimiter and emit every
    // interior line verbatim, so fence-like content is not rewritten.
    for (index, cached) in block.lines.into_iter().enumerate() {
        let emitted = if index == 0 {
            cached.compressed.unwrap_or(cached.line)
        } else {
            cached.line
        };
        out.push(emitted);
    }
}

fn flush_matched_block(block: PendingFenceBlock, out: &mut Vec<String>) {
    let rewrite = opening_rewrite(block.has_conflicting_interior_fence);
    let closing_index = block.lines.len() - 1;
    for (index, cached) in block.lines.into_iter().enumerate() {
        let emitted = if index == 0 || index == closing_index {
            rewrite_delimiter(cached, rewrite)
        } else {
            cached.line
        };
        out.push(emitted);
    }
}

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
    line: &'a str,
    observation: FenceObservation,
    fence: Option<(&'a str, &'a str, &'a str)>,
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
            compressed: compressed_fence_line(line),
        }
    }

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
/// `flush_unmatched_block`, which normalizes only the opening delimiter and
/// emits every interior line verbatim, so fence-like content inside that
/// unclosed block is preserved rather than rewritten.
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

/// Attach orphaned language specifiers to opening fences.
///
/// After compressing fences, a language may appear on its own line directly
/// before a fence. This function removes that line and applies the specifier
/// to the following opening fence, dropping any intervening blank lines when
/// attachment succeeds. When the fence is unindented, the specifier's
/// indentation is used. If the specifier's indentation extends the fence's, the
/// deeper indentation is retained.
///
/// Specifiers containing spaces are accepted and normalized. Fences labelled
/// `null` are normalized to empty by `compress_fences`, so only empty languages
/// are treated as absent.
///
/// # Examples
///
/// ```
/// use mdtablefix::fences::{attach_orphan_specifiers, compress_fences};
/// let lines = vec![
///     "Rust".to_string(),
///     "```".to_string(),
///     "fn main() {}".to_string(),
///     "```".to_string(),
/// ];
/// let fixed = attach_orphan_specifiers(&compress_fences(&lines));
/// assert_eq!(fixed[0], "```rust");
/// ```
#[must_use]
pub fn attach_orphan_specifiers(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut tracker = FenceTracker::new();
    let mut lines = lines.iter().peekable();

    while let Some(line) = lines.next() {
        let fence = tracker.observe_source_line(line);
        if fence.was_in_fence {
            out.push(line.clone());
            continue;
        }

        let (spec, indent) = normalize_specifier(line);
        if ORPHAN_LANG_RE.is_match(&spec) && out.last().is_none_or(|l: &String| l.trim().is_empty())
        {
            attach_to_next_fence(&mut lines, &spec, &indent, &mut out, line, &mut tracker);
            continue;
        }

        out.push(line.clone());
    }

    out
}
