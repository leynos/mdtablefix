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

/// Check whether a line contains only a valid orphan specifier.
///
/// # Examples
///
/// ```rust,ignore
/// use mdtablefix::fences::is_orphan_specifier;
/// assert!(is_orphan_specifier("Rust"));
/// assert!(is_orphan_specifier("TOML, Ini"));
/// assert!(!is_orphan_specifier("rust?"));
/// ```
fn is_orphan_specifier(line: &str) -> bool {
    let (clean, _) = normalize_specifier(line);
    ORPHAN_LANG_RE.is_match(&clean)
}

/// Determine whether a line is an opening fence lacking a language.
///
/// # Examples
///
/// ```rust,ignore
/// use mdtablefix::fences::is_opening_fence_without_specifier;
/// assert!(is_opening_fence_without_specifier("```") );
/// assert!(!is_opening_fence_without_specifier("```rust"));
/// ```
fn is_opening_fence_without_specifier(line: &str) -> bool {
    if let Some(cap) = FENCE_RE.captures(line) {
        let lang = cap.get(3).map_or("", |m| m.as_str());
        is_null_lang(lang)
    } else {
        false
    }
}

/// Combine an opening fence with a language specifier.
///
/// Uses the specifier's indentation when the fence is unindented.
///
/// # Examples
///
/// ```rust,ignore
/// use mdtablefix::fences::attach_specifier_to_fence;
/// assert_eq!(attach_specifier_to_fence("```", "rust", "  "), "  ```rust");
/// ```
fn attach_specifier_to_fence(fence_line: &str, specifier: &str, spec_indent: &str) -> String {
    let fence_indent = FENCE_RE
        .captures(fence_line)
        .and_then(|cap| cap.get(1))
        .map_or("", |m| m.as_str());
    let final_indent = if fence_indent.is_empty() {
        spec_indent
    } else {
        fence_indent
    };
    format!("{final_indent}```{specifier}")
}

/// Attach orphaned language specifiers to opening fences.
///
/// After compressing fences, an orphaned specifier may remain as a single word
/// on the line before a fence. This function removes that line and applies the
/// specifier to the following opening fence. Indentation from the specifier
/// line is preserved when the fence itself is unindented. Specifiers containing
/// spaces are accepted and normalized.
/// Fences labelled `null` are normalized to empty by `compress_fences`,
/// so only empty languages are treated as absent.
///
/// # Panics
///
/// Panics if a fence line disappears between peeking and consumption.
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
    enum State {
        LookingForSpecifier,
        InsideFence,
    }
    let mut out = Vec::with_capacity(lines.len());
    let mut iter = lines.iter().peekable();
    let mut state = State::LookingForSpecifier;

    while let Some(line) = iter.next() {
        match state {
            State::LookingForSpecifier => {
                if is_orphan_specifier(line)
                    && out.last().is_none_or(|l: &String| l.trim().is_empty())
                {
                    let (spec, indent) = normalize_specifier(line);
                    let mut lookahead = iter.clone();
                    let mut blanks = 0;
                    while let Some(next) = lookahead.peek() {
                        if next.trim().is_empty() {
                            blanks += 1;
                            lookahead.next();
                        } else {
                            break;
                        }
                    }
                    if lookahead
                        .peek()
                        .is_some_and(|n| is_opening_fence_without_specifier(n))
                    {
                        for _ in 0..blanks {
                            iter.next();
                        }
                        let fence = iter.next().expect("peeked fence");
                        out.push(attach_specifier_to_fence(fence, &spec, &indent));
                        state = State::InsideFence;
                        continue;
                    }
                }
                if FENCE_RE.captures(line).is_some() {
                    state = State::InsideFence;
                }
                out.push(line.clone());
            }
            State::InsideFence => {
                if FENCE_RE.captures(line).is_some() {
                    state = State::LookingForSpecifier;
                }
                out.push(line.clone());
            }
        }
    }
    out
}
