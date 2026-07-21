//! Pre-processing utilities for normalizing fenced code block delimiters.
//!
//! `compress_fences` reduces safe outer delimiters to three backticks while
//! preserving nested fence-like content whose marker runs are literal text.
//! The local `FENCE_RE` defines which delimiter lines this module can
//! normalize, while `wrap::is_fence` and `FenceTracker` provide the structural
//! Markdown fence semantics shared with wrapping.
//! `attach_orphan_specifiers` then finds orphaned fence specifier lines and
//! attaches them to the following fence, preserving the retained indentation
//! and normalized language specifier.
use std::sync::LazyLock;

use regex::Regex;

use crate::wrap::{FenceObservation, FenceTracker, is_fence};

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
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
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

struct PendingFenceBlock {
    opening_marker: String,
    /// The opening line rewritten with a compressed delimiter, cached at parse
    /// time so the closing decision and `flush_matched_block` need not re-parse
    /// it. `None` when the opening line is not rewritable.
    opening_compressed: Option<String>,
    has_conflicting_interior_fence: bool,
    lines: Vec<String>,
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

/// Rewrite a fence delimiter, reusing a cached compressed rewrite when one was
/// computed for this line at parse time.
///
/// The `PreserveDelimiters` strategy is only chosen once per block, so its
/// rewrite is computed on demand rather than cached.
fn rewrite_cached_fence_line(
    line: &str,
    rewrite: FenceRewrite,
    compressed: Option<&str>,
) -> String {
    match rewrite {
        FenceRewrite::Compress => compressed.map_or_else(|| line.to_owned(), str::to_owned),
        FenceRewrite::PreserveDelimiters => {
            preserved_fence_line(line).unwrap_or_else(|| line.to_owned())
        }
    }
}
fn flush_unmatched_block(block: PendingFenceBlock, out: &mut Vec<String>) {
    out.extend(
        block
            .lines
            .into_iter()
            .map(|line| compressed_fence_line(&line).unwrap_or(line)),
    );
}

fn flush_matched_block(
    block: PendingFenceBlock,
    closing_compressed: Option<&str>,
    out: &mut Vec<String>,
) {
    let rewrite = opening_rewrite(block.has_conflicting_interior_fence);
    let PendingFenceBlock {
        opening_compressed,
        lines,
        ..
    } = block;
    let closing_index = lines.len() - 1;
    for (index, line) in lines.into_iter().enumerate() {
        let rewritten = if index == 0 {
            rewrite_cached_fence_line(&line, rewrite, opening_compressed.as_deref())
        } else if index == closing_index {
            rewrite_cached_fence_line(&line, rewrite, closing_compressed)
        } else {
            line
        };
        out.push(rewritten);
    }
}

fn flush_original_block(block: PendingFenceBlock, out: &mut Vec<String>) {
    out.extend(block.lines);
}

/// Emit a completed block, rewriting its delimiters when both ends are safely
/// rewritable and otherwise preserving the original lines.
fn flush_completed_block(
    block: PendingFenceBlock,
    closing_compressed: Option<&str>,
    out: &mut Vec<String>,
) {
    if block.opening_compressed.is_some() && closing_compressed.is_some() {
        flush_matched_block(block, closing_compressed, out);
    } else {
        flush_original_block(block, out);
    }
}

/// Begin a pending fence block for a line observed outside any active fence.
///
/// Any block that was still pending is emitted verbatim first. When `line`
/// opens a fence a fresh block is returned; otherwise the (possibly compressed)
/// line is pushed to `out` and `None` is returned.
fn start_fence_block(
    previous: Option<PendingFenceBlock>,
    line: &str,
    parsed: Option<(&str, &str, &str)>,
    compressed: Option<String>,
    out: &mut Vec<String>,
) -> Option<PendingFenceBlock> {
    if let Some(block) = previous {
        flush_original_block(block, out);
    }
    let Some((_indent, opening_marker, _info)) = parsed else {
        out.push(compressed.unwrap_or_else(|| line.to_owned()));
        return None;
    };
    Some(PendingFenceBlock {
        opening_marker: opening_marker.to_owned(),
        opening_compressed: compressed,
        has_conflicting_interior_fence: false,
        lines: vec![line.to_owned()],
    })
}

/// Advance the pending fence block for a line observed inside an active fence,
/// returning the block that remains pending afterwards (if any).
///
/// Interior fence markers are accumulated as literal content until the block
/// closes at its opening depth, at which point it is flushed.
fn advance_fence_block(
    pending: Option<PendingFenceBlock>,
    line: &str,
    fence: FenceObservation,
    parsed: Option<(&str, &str, &str)>,
    compressed: Option<&str>,
    out: &mut Vec<String>,
) -> Option<PendingFenceBlock> {
    let Some(mut block) = pending else {
        out.push(line.to_owned());
        return None;
    };

    if fence.is_fence_marker
        && fence.is_in_fence
        && interior_fence_requires_preserved_delimiters(&block.opening_marker, parsed)
    {
        block.has_conflicting_interior_fence = true;
    }
    block.lines.push(line.to_owned());

    if !fence.is_fence_marker || fence.is_in_fence {
        return Some(block);
    }

    flush_completed_block(block, compressed, out);
    None
}
pub fn compress_fences(lines: &[String]) -> Vec<String> {
    let mut tracker = FenceTracker::new();
    let mut pending_block = None;
    let mut out = Vec::with_capacity(lines.len());

    for line in lines {
        let fence = tracker.observe_source_line(line);
        // Parse the fence structure and compressed rewrite once per raw line,
        // then reuse them for every opening and closing decision below.
        let parsed = is_fence(line);
        let compressed = compressed_fence_line(line);

        pending_block = if fence.was_in_fence {
            advance_fence_block(
                pending_block.take(),
                line,
                fence,
                parsed,
                compressed.as_deref(),
                &mut out,
            )
        } else {
            start_fence_block(pending_block.take(), line, parsed, compressed, &mut out)
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
