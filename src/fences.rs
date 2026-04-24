//! Pre-processing utilities for normalizing fenced code block delimiters.
//!
//! `compress_fences` reduces safe outer delimiters to three backticks while
//! preserving nested fence-like content whose marker runs are literal text.
//! The local `FENCE_RE` defines which delimiter lines this module can
//! normalize, while `wrap::is_fence` and `FenceTracker` provide the structural
//! Markdown fence semantics shared with wrapping.
use std::sync::LazyLock;

use regex::Regex;

use crate::wrap::{FenceTracker, is_fence};

static FENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)(`{3,}|~{3,})([A-Za-z0-9_+.,-]*)\s*$").unwrap());

static ORPHAN_LANG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9_+.-]*[A-Za-z0-9_+\-](?:,[A-Za-z0-9_+.-]*[A-Za-z0-9_+\-])*$").unwrap()
});

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

fn interior_fence_requires_preserved_delimiters(opening_marker: &str, line: &str) -> bool {
    let Some((_indent, marker, _info)) = is_fence(line) else {
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

fn rewrite_fence_line(line: &str, rewrite: FenceRewrite) -> String {
    match rewrite {
        FenceRewrite::Compress => compressed_fence_line(line).unwrap_or_else(|| line.to_owned()),
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

fn flush_matched_block(block: PendingFenceBlock, out: &mut Vec<String>) {
    let rewrite = opening_rewrite(block.has_conflicting_interior_fence);
    let closing_index = block.lines.len() - 1;
    for (index, line) in block.lines.into_iter().enumerate() {
        if index == 0 || index == closing_index {
            out.push(rewrite_fence_line(&line, rewrite));
        } else {
            out.push(line);
        }
    }
}

/// Compress safe outer fences to exactly three backticks.
///
/// Lines that do not start with backtick fences are returned unchanged.
/// Fence-like lines inside a wider fenced block are literal content and are
/// returned unchanged. An outer delimiter is also preserved when shortening or
/// changing it would make an inner literal fence line look structural.
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
        if !tracker.in_fence() {
            if FENCE_RE.captures(line).is_none() {
                out.push(line.clone());
                continue;
            }
            let Some((_indent, opening_marker, _info)) = is_fence(line) else {
                out.push(compressed_fence_line(line).unwrap_or_else(|| line.clone()));
                continue;
            };
            let _ = tracker.observe(line);
            pending_block = Some(PendingFenceBlock {
                opening_marker: opening_marker.to_owned(),
                has_conflicting_interior_fence: false,
                lines: vec![line.clone()],
            });
            continue;
        }

        let Some(mut block) = pending_block.take() else {
            out.push(line.clone());
            continue;
        };

        let observed_fence = tracker.observe(line);
        if observed_fence
            && tracker.in_fence()
            && interior_fence_requires_preserved_delimiters(&block.opening_marker, line)
        {
            block.has_conflicting_interior_fence = true;
        }
        block.lines.push(line.clone());

        if !observed_fence {
            pending_block = Some(block);
            continue;
        }

        if tracker.in_fence() {
            pending_block = Some(block);
            continue;
        }

        flush_matched_block(block, &mut out);
    }

    if let Some(block) = pending_block {
        flush_unmatched_block(block, &mut out);
    }

    out
}

/// Combine an opening fence with a language specifier.
///
/// The fence's indentation is retained whenever present. If the specifier's
/// indentation extends the fence's, the deeper specifier indentation is used.
/// When the fence lacks indentation, the specifier's indentation becomes the fence's.
/// If the indentations differ without one extending the other (e.g., tabs vs spaces),
/// the fence's indentation wins.
///
/// # Examples
///
/// ```rust,ignore
/// use mdtablefix::fences::attach_specifier_to_fence;
/// assert_eq!(attach_specifier_to_fence("```", "rust", "  "), "  ```rust");
/// assert_eq!(attach_specifier_to_fence("  ```", "rust", "    "), "    ```rust");
/// ```
fn attach_specifier_to_fence(fence_line: &str, specifier: &str, spec_indent: &str) -> String {
    let Some(cap) = FENCE_RE.captures(fence_line) else {
        return fence_line.to_owned();
    };
    let fence_indent = cap.get(1).map_or("", |m| m.as_str());
    let fence_marker = cap.get(2).map_or("```", |m| m.as_str());
    let final_indent = if fence_indent.is_empty() || spec_indent.starts_with(fence_indent) {
        spec_indent
    } else {
        fence_indent
    };
    format!("{final_indent}{fence_marker}{specifier}")
}

fn orphan_specifier_target(lines: &[String], start: usize) -> Option<usize> {
    let mut index = start;
    while index < lines.len() && lines[index].trim().is_empty() {
        index += 1;
    }
    if index >= lines.len() || FENCE_RE.captures(&lines[index]).is_none() {
        return None;
    }
    Some(index)
}

fn orphan_specifier_target_without_language(lines: &[String], start: usize) -> Option<usize> {
    let target = orphan_specifier_target(lines, start)?;
    let cap = FENCE_RE.captures(&lines[target])?;
    let lang = cap.get(3).map_or("", |m| m.as_str());
    is_null_lang(lang).then_some(target)
}

/// Attach orphaned language specifiers to opening fences.
///
/// After compressing fences, a language may appear on its own line directly
/// before a fence. This function removes that line and applies the specifier
/// to the following opening fence. Blank lines between the specifier and the
/// fence are skipped. When the fence is unindented, the specifier's indentation
/// is used. If the specifier's indentation extends the fence's, the deeper
/// indentation is retained.
///
/// Specifiers containing spaces are accepted and normalised. Fences labelled
/// `null` are normalised to empty by `compress_fences`, so only empty languages
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
    let mut i = 0;

    while i < lines.len() {
        let line = &lines[i];
        if tracker.in_fence() {
            let _ = tracker.observe(line);
            out.push(line.clone());
            i += 1;
            continue;
        }

        let (spec, indent) = normalize_specifier(line);
        if ORPHAN_LANG_RE.is_match(&spec) && out.last().is_none_or(|l: &String| l.trim().is_empty())
        {
            if let Some(target) = orphan_specifier_target_without_language(lines, i + 1) {
                out.push(attach_specifier_to_fence(&lines[target], &spec, &indent));
                let _ = tracker.observe(&lines[target]);
                i = target + 1;
                continue;
            }
            out.push(line.clone());
            let _ = tracker.observe(line);
            i += 1;
            continue;
        }

        out.push(line.clone());
        let _ = tracker.observe(line);
        i += 1;
    }

    out
}
