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
    let fence_indent = FENCE_RE
        .captures(fence_line)
        .and_then(|cap| cap.get(1))
        .map_or("", |m| m.as_str());
    let final_indent = if fence_indent.is_empty() || spec_indent.starts_with(fence_indent) {
        spec_indent
    } else {
        fence_indent
    };
    format!("{final_indent}```{specifier}")
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
    let mut i = 0;

    while i < lines.len() {
        let line = &lines[i];
        let (spec, indent) = normalize_specifier(line);
        if ORPHAN_LANG_RE.is_match(&spec) && out.last().is_none_or(|l: &String| l.trim().is_empty())
        {
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if j < lines.len()
                && let Some(cap) = FENCE_RE.captures(&lines[j])
            {
                let lang = cap.get(3).map_or("", |m| m.as_str());
                if is_null_lang(lang) {
                    out.push(attach_specifier_to_fence(&lines[j], &spec, &indent));
                    i = j + 1;
                    continue;
                }
            }
            out.push(line.clone());
            i += 1;
            continue;
        }

        out.push(line.clone());
        i += 1;
    }

    out
}
