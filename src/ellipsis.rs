//! Replace sequences of three dots with the ellipsis character.
//!
//! Groups of three consecutive dots become a single Unicode ellipsis. Longer
//! runs are processed left-to-right so trailing dots that do not form a
//! complete triple remain. Fenced code blocks and inline code spans are left
//! untouched.

use std::sync::LazyLock;

use regex::Regex;

use crate::wrap::{Token, tokenize_markdown};

static DOT_RE: LazyLock<Regex> = lazy_regex!(r"\.{3,}", "valid dot regex");

/// Replace `...` with `…` outside code spans and fences.
#[must_use]
pub fn replace_ellipsis(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }
    let joined = lines.join("\n");
    let mut out = String::new();
    for token in tokenize_markdown(&joined) {
        match token {
            Token::Text(t) => {
                let replaced = DOT_RE.replace_all(t, |caps: &regex::Captures<'_>| {
                    let len = caps[0].len();
                    let ellipses = "…".repeat(len / 3);
                    let leftover = ".".repeat(len % 3);
                    format!("{ellipses}{leftover}")
                });
                out.push_str(&replaced);
            }
            Token::Code(c) => {
                out.push('`');
                out.push_str(c);
                out.push('`');
            }
            Token::Fence(f) => {
                out.push_str(f);
            }
            Token::Newline => out.push('\n'),
        }
    }
    out.split('\n').map(str::to_string).collect()
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

    #[test]
    fn handles_empty_input() {
        let input: Vec<String> = Vec::new();
        let expected: Vec<String> = Vec::new();
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn handles_multiple_fenced_blocks() {
        let input = vec![
            "text...".to_string(),
            "```".to_string(),
            "code...".to_string(),
            "```".to_string(),
            "more text...".to_string(),
        ];
        let expected = vec![
            "text…".to_string(),
            "```".to_string(),
            "code...".to_string(),
            "```".to_string(),
            "more text…".to_string(),
        ];
        assert_eq!(replace_ellipsis(&input), expected);
    }
}
