//! Replace sequences of three dots with the ellipsis character.
//!
//! This module provides a helper for normalising textual ellipses. It respects
//! fenced code blocks and inline code spans so that code is left untouched.

use crate::wrap::{is_fence, tokenize_markdown};

/// Replace `...` with `…` outside code spans and fences.
#[must_use]
pub fn replace_ellipsis(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut in_code = false;
    for line in lines {
        if is_fence(line) {
            in_code = !in_code;
            out.push(line.clone());
            continue;
        }
        if in_code {
            out.push(line.clone());
            continue;
        }
        let mut replaced = String::new();
        for token in tokenize_markdown(line) {
            if token.starts_with('`') {
                replaced.push_str(&token);
            } else {
                replaced.push_str(&token.replace("...", "…"));
            }
        }
        out.push(replaced);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_simple_text() {
        let input = vec!["wait...".to_string()];
        let expected = vec!["wait…".to_string()];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn ignores_code_spans() {
        let input = vec!["a `b...` c".to_string()];
        let expected = input.clone();
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn ignores_fenced_blocks() {
        let input = vec!["```".to_string(), "...".to_string(), "```".to_string()];
        let expected = input.clone();
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn replaces_long_sequences() {
        let input = vec![".... ..... ...... .......".to_string()];
        let expected = vec!["…. ….. …… …….".to_string()];
        assert_eq!(replace_ellipsis(&input), expected);
    }
}
