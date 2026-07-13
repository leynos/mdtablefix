//! Replace sequences of three dots with the ellipsis character.
//!
//! Groups of three consecutive dots become a single Unicode ellipsis. Longer
//! runs are processed left-to-right so trailing dots that do not form a
//! complete triple remain. Fenced and indented code blocks, plus inline code
//! spans, are left untouched.

use std::sync::LazyLock;

use regex::Regex;

use crate::{
    textproc::{Token, process_tokens, push_original_token},
    wrap::{FenceTracker, leading_indent},
};

static DOT_RE: LazyLock<Regex> = lazy_regex!(r"\.{3,}", "ellipsis pattern regex should compile");

/// Tracks whether a line belongs to a top-level indented code block.
///
/// This state is deliberately local to ellipsis replacement. Wrapping has its
/// own block classifier, while this pass only needs to decide which original
/// lines must remain byte-for-byte unchanged.
#[derive(Debug)]
struct IndentedCodeTracker {
    is_in_block: bool,
    may_start_block: bool,
}

impl Default for IndentedCodeTracker {
    fn default() -> Self {
        Self {
            is_in_block: false,
            may_start_block: true,
        }
    }
}

impl IndentedCodeTracker {
    fn observe(&mut self, line: &str) -> bool {
        if line.trim().is_empty() {
            self.may_start_block = true;
            return self.is_in_block;
        }

        let (indent_width, _) = leading_indent(line);
        let is_indented = indent_width >= 4;
        let belongs_to_block = is_indented && (self.is_in_block || self.may_start_block);

        self.is_in_block = belongs_to_block;
        self.may_start_block = false;
        belongs_to_block
    }
}

fn replace_ellipsis_in_prose(line: &str) -> String {
    process_tokens(&[line.to_owned()], |tok, out| match tok {
        Token::Text(text) => {
            if !DOT_RE.is_match(text) {
                out.push_str(text);
                return;
            }
            let replaced = DOT_RE.replace_all(text, |caps: &regex::Captures<'_>| {
                let len = caps[0].len();
                let ellipses = "…".repeat(len / 3);
                let leftover = ".".repeat(len % 3);
                format!("{ellipses}{leftover}")
            });
            out.push_str(&replaced);
        }
        _ => push_original_token(&tok, out),
    })
    .into_iter()
    .next()
    .unwrap_or_default()
}

/// Replace `...` with `…` outside code spans and code blocks.
#[must_use]
pub fn replace_ellipsis(lines: &[String]) -> Vec<String> {
    let mut fence_tracker = FenceTracker::default();
    let mut indented_code_tracker = IndentedCodeTracker::default();

    lines
        .iter()
        .map(|line| {
            let is_indented_code = indented_code_tracker.observe(line);
            let is_fence = fence_tracker.observe(line);
            if is_fence || fence_tracker.in_fence() || is_indented_code {
                line.clone()
            } else {
                replace_ellipsis_in_prose(line)
            }
        })
        .collect()
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
    fn ignores_indented_code_blocks() {
        let input = vec![
            "Expected output:".to_string(),
            String::new(),
            "    running 2 tests".to_string(),
            "    test foo ... ok".to_string(),
            String::new(),
            "    ...".to_string(),
            "after...".to_string(),
        ];
        let expected = vec![
            "Expected output:".to_string(),
            String::new(),
            "    running 2 tests".to_string(),
            "    test foo ... ok".to_string(),
            String::new(),
            "    ...".to_string(),
            "after…".to_string(),
        ];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn indented_code_cannot_interrupt_a_paragraph() {
        let input = vec!["paragraph".to_string(), "    prose...".to_string()];
        let expected = vec!["paragraph".to_string(), "    prose…".to_string()];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[rstest::rstest]
    #[case::three_spaces("   ...", "   …")]
    #[case::four_spaces("    ...", "    ...")]
    #[case::one_tab("\t...", "\t...")]
    fn observes_indented_code_boundary(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(
            replace_ellipsis(&[input.to_string()]),
            [expected.to_string()]
        );
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
