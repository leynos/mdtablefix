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

use crate::wrap::FenceTracker;

mod attachment;
mod compress;

#[cfg(test)]
#[path = "fences_properties.rs"]
mod properties;

use attachment::attach_to_next_fence;
pub use compress::compress_fences;

/// Parses a complete fence delimiter, retaining indentation, marker, and language specifier.
static FENCE_RE: LazyLock<Regex> = lazy_regex!(
    r"^(\s*)(`{3,}|~{3,})([A-Za-z0-9_+.,-]*)\s*$",
    "fence delimiter and language specifier pattern should compile",
);

/// Recognises standalone language-specifier lines eligible for fence attachment.
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

        if attachment::preserve_thematic_break(line, &mut out) {
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
