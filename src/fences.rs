//! Pre-processing utilities for normalising fenced code block delimiters.
//!
//! `compress_fences` reduces any sequence of three or more backticks
//! followed by optional language identifiers to exactly three backticks.
//! It preserves indentation and the language list.
use std::sync::LazyLock;

use regex::Regex;

static BACKTICK_FENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)`{3,}([A-Za-z0-9_+.,-]*)\s*$").unwrap());

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
            if let Some(cap) = BACKTICK_FENCE_RE.captures(line) {
                let indent = cap.get(1).map_or("", |m| m.as_str());
                let lang = cap.get(2).map_or("", |m| m.as_str());
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
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut in_fence = false;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_fence {
                in_fence = false;
            } else {
                if trimmed == "```"
                    && let Some(idx) = out
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, l)| !l.trim().is_empty() && ORPHAN_LANG_RE.is_match(l.trim()))
                        .map(|(i, _)| i)
                {
                    let lang = out[idx].trim().to_string();
                    while out.len() > idx + 1 && out[idx + 1].trim().is_empty() {
                        out.remove(idx + 1);
                    }
                    out.remove(idx);
                    out.push(format!("```{lang}"));
                    in_fence = true;
                    continue;
                }
                in_fence = true;
            }
        }
        out.push(line.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compresses_simple_fence() {
        let input = vec![
            "````rust".to_string(),
            "code".to_string(),
            "````".to_string(),
        ];
        let out = compress_fences(&input);
        assert_eq!(out[0], "```rust");
        assert_eq!(out[2], "```");
    }

    #[test]
    fn compresses_indented_fence() {
        let input = vec!["    `````foo,bar   ".to_string()];
        let out = compress_fences(&input);
        assert_eq!(out[0], "    ```foo,bar");
    }

    #[test]
    fn ignores_tilde_fence() {
        let input = vec!["~~~~".to_string()];
        let out = compress_fences(&input);
        assert_eq!(out, input);
    }

    #[test]
    fn attaches_orphan_specifier() {
        let input = vec![
            "Rust".to_string(),
            "```".to_string(),
            "fn main() {}".to_string(),
            "```".to_string(),
        ];
        let out = attach_orphan_specifiers(&compress_fences(&input));
        assert_eq!(
            out,
            vec![
                "```Rust".to_string(),
                "fn main() {}".to_string(),
                "```".to_string(),
            ]
        );
    }
}
