//! Pre-processing utilities for normalizing fenced code block delimiters.
//!
//! `compress_fences` reduces any sequence of three or more backticks or
//! tildes followed by optional language identifiers to exactly three
//! backticks.
//! It preserves indentation and the language list.
use std::sync::LazyLock;

use regex::Regex;

static FENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)(`{3,}|~{3,})([A-Za-z0-9_+.,-]*)\s*$").unwrap());

static ORPHAN_LANG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_+.-]+(?:,[A-Za-z0-9_+.-]+)*$").unwrap());

/// Compress backtick fences to exactly three backticks.
///
/// Lines that do not start with backtick fences are returned unchanged.
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
    lines
        .iter()
        .map(|line| {
            if let Some(cap) = FENCE_RE.captures(line) {
                let indent = cap.get(1).map_or("", |m| m.as_str());
                let lang = cap.get(3).map_or("", |m| m.as_str());
                if lang.is_empty() {
                    format!("{indent}```")
                } else {
                    format!("{indent}```{lang}")
                }
            } else {
                line.clone()
            }
        })
        .collect()
}

/// Attach orphaned language specifiers to opening fences.
///
/// After compressing fences, an orphaned specifier may remain as a single word
/// on the line before a fence. This function removes that line and applies the
/// specifier to the following opening fence.
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
/// assert_eq!(fixed[0], "```Rust");
/// ```
#[must_use]
pub fn attach_orphan_specifiers(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut in_fence = false;
    let mut iter = lines.iter().peekable();

    while let Some(line) = iter.next() {
        let trimmed = line.trim();

        if !in_fence && ORPHAN_LANG_RE.is_match(trimmed) {
            let mut peek_ahead = iter.clone();
            let mut found_fence = false;

            while let Some(next_line) = peek_ahead.peek() {
                let next_trimmed = next_line.trim();
                if next_trimmed.is_empty() {
                    peek_ahead.next();
                } else if next_trimmed == "```" {
                    found_fence = true;
                    break;
                } else {
                    break;
                }
            }

            if found_fence {
                while let Some(next_line) = iter.peek() {
                    if next_line.trim().is_empty() {
                        iter.next();
                    } else {
                        break;
                    }
                }
                iter.next();
                out.push(format!("```{trimmed}"));
                in_fence = true;
                continue;
            }
        }

        if trimmed.starts_with("```") {
            in_fence = !in_fence;
        }

        out.push(line.clone());
    }
    out
}
