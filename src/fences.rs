//! Pre-processing utilities for normalising fenced code block delimiters.
//!
//! `compress_fences` reduces any sequence of three or more backticks or
//! tildes followed by optional language identifiers to exactly three
//! backticks.
//! It preserves indentation and the language list.
use std::sync::LazyLock;

use regex::Regex;

static FENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)(`{3,}|~{3,})([A-Za-z0-9_+.,-]*)\s*$").unwrap());

static ORPHAN_LANG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9_+.-]*[A-Za-z0-9_+\-](?:,[A-Za-z0-9_+.-]*[A-Za-z0-9_+\-])*$").unwrap()
});

/// Determine whether a language specifier denotes an absent language.
///
/// A language is absent when it is empty or the case-insensitive string `null`, with surrounding whitespace ignored.
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
                if is_null_lang(lang) {
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
/// specifier to the following opening fence. Indentation from the specifier
/// line is preserved when the fence itself is unindented. Specifiers containing
/// spaces are accepted and normalised.
/// Fences labelled `null` are normalised to empty by `compress_fences`,
/// so only empty languages are treated as absent.
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
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut in_fence = false;
    for line in lines {
        if let Some(cap) = FENCE_RE.captures(line) {
            if in_fence {
                in_fence = false;
                out.push(line.clone());
                continue;
            }

            let indent = cap.get(1).map_or("", |m| m.as_str());
            let lang_present = cap.get(3).map_or("", |m| m.as_str());

            if lang_present.is_empty() {
                let mut idx = out.len();
                while idx > 0 && out[idx - 1].trim().is_empty() {
                    idx -= 1;
                }
                if idx > 0 {
                    let (candidate_clean, candidate_indent) = normalize_specifier(&out[idx - 1]);
                    if ORPHAN_LANG_RE.is_match(&candidate_clean)
                        && (idx == 1 || out[idx - 2].trim().is_empty())
                    {
                        let final_indent = if indent.is_empty() {
                            candidate_indent.as_str()
                        } else {
                            indent
                        };
                        out.truncate(idx - 1);
                        out.push(format!("{final_indent}```{candidate_clean}"));
                        in_fence = true;
                        continue;
                    }
                }
                in_fence = true;
                out.push(format!("{indent}```"));
                continue;
            }

            in_fence = true;
            out.push(line.clone());
            continue;
        }

        out.push(line.clone());
    }
    out
}
