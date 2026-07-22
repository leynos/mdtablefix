//! Replace sequences of three dots with the ellipsis character.
//!
//! Groups of three consecutive dots become a single Unicode ellipsis. Longer
//! runs are processed left-to-right so trailing dots that do not form a
//! complete triple remain. Fenced and indented code blocks, plus inline code
//! spans, are left untouched.

use std::sync::LazyLock;

use regex::Regex;
use tracing::trace;

use crate::{
    textproc::{Token, push_original_token, tokenize_markdown},
    wrap::{
        BlockKind,
        FenceTracker,
        LinkReferenceMatcher,
        LinkTitleWindow,
        LinkTitleWindowOutcome,
        classify_block,
        leading_indent,
    },
};

mod protected;

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
    fn observe(&mut self, line: &str, completes_leaf_block: bool) -> bool {
        if line.trim().is_empty() {
            self.may_start_block = true;
            return self.is_in_block;
        }

        let (indent_width, _) = leading_indent(line);
        let is_indented = indent_width >= 4;
        let belongs_to_block = is_indented && (self.is_in_block || self.may_start_block);

        self.is_in_block = belongs_to_block;
        // A paragraph prevents indented code from starting on the next line.
        // Complete leaf blocks, by contrast, leave no paragraph open.
        self.may_start_block = completes_leaf_block;
        if belongs_to_block {
            trace!(
                width = indent_width,
                reason = "indented_code",
                "preserving ellipsis input line verbatim"
            );
        }
        belongs_to_block
    }

    fn observe_completed_block(&mut self) {
        self.is_in_block = false;
        self.may_start_block = true;
    }
}

fn completes_leaf_block(block_kind: Option<BlockKind>) -> bool {
    matches!(
        block_kind,
        Some(
            BlockKind::Heading
                | BlockKind::LinkReferenceDefinition
                | BlockKind::MarkdownlintDirective
        )
    )
}

fn replace_ellipsis_in_prose(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for token in tokenize_markdown(line) {
        match token {
            Token::Text(text) => replace_text_ellipsis(text, &mut out),
            _ => push_original_token(&token, &mut out),
        }
    }
    out
}

fn replace_text_ellipsis(text: &str, out: &mut String) {
    let mut cursor = 0;
    for span in protected::literal_spans(text) {
        replace_dot_runs(&text[cursor..span.start], out);
        out.push_str(&text[span.clone()]);
        cursor = span.end;
    }
    replace_dot_runs(&text[cursor..], out);
}

fn replace_dot_runs(text: &str, out: &mut String) {
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

/// Replace `...` with `…` outside code spans and code blocks.
#[must_use]
pub fn replace_ellipsis(lines: &[String]) -> Vec<String> {
    let mut fence_tracker = FenceTracker::default();
    let mut indented_code_tracker = IndentedCodeTracker::default();
    let link_matcher = LinkReferenceMatcher::production();
    let mut link_title_window = LinkTitleWindow::default();

    lines
        .iter()
        .map(|line| {
            let fence = fence_tracker.observe_source_line(line);
            if fence.is_fence_marker || fence.is_in_fence {
                indented_code_tracker.observe_completed_block();
                link_title_window.observe_fence_context();
                return line.clone();
            }

            let continuation_outcome = link_title_window.observe_next_line(line, link_matcher);
            if continuation_outcome == Some(LinkTitleWindowOutcome::EmitVerbatim) {
                indented_code_tracker.observe_completed_block();
                trace!(
                    kind = ?continuation_outcome,
                    reason = "link_reference_continuation",
                    "preserving ellipsis input line verbatim"
                );
                return line.clone();
            }

            let block_kind = classify_block(line, link_matcher);
            let is_indented_code =
                indented_code_tracker.observe(line, completes_leaf_block(block_kind));
            if matches!(block_kind, Some(BlockKind::LinkReferenceDefinition)) {
                link_title_window.observe_definition(line, link_matcher);
                trace!(
                    kind = ?link_title_window,
                    reason = "link_reference_definition",
                    "preserving ellipsis input line verbatim"
                );
                return line.clone();
            }

            if is_indented_code {
                return line.clone();
            }

            replace_ellipsis_in_prose(line)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Unit tests for ellipsis replacement.

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
    fn ignores_blockquoted_fenced_blocks() {
        // The depth-aware fence tracker recognises a fence opened inside a
        // blockquote, so the enclosed `...` stays literal.
        let input = vec![
            "> ```".to_string(),
            "> ...".to_string(),
            "> ```".to_string(),
        ];
        let expected = input.clone();
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[rstest::rstest]
    #[case::code_block(
        &["Expected output:", "", "    running 2 tests", "    test foo ... ok", "", "    ...", "after..."],
        &["Expected output:", "", "    running 2 tests", "    test foo ... ok", "", "    ...", "after…"]
    )]
    #[case::paragraph_interruption(&["paragraph", "    prose..."], &["paragraph", "    prose…"])]
    fn transforms_indented_lines(#[case] input: &[&str], #[case] expected: &[&str]) {
        let input = input.iter().map(ToString::to_string).collect::<Vec<_>>();
        let expected = expected.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[rstest::rstest]
    #[case::heading(&["# Heading", "    literal..."])]
    #[case::closed_fence(&["```", "fenced...", "```", "    literal..."])]
    fn completed_blocks_allow_following_indented_code(#[case] input: &[&str]) {
        let input = input.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(replace_ellipsis(&input), input);
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

    #[rstest::rstest]
    #[case::inline_link("[wait...](https://example.com/a...b)")]
    #[case::image("![alt...](images/a...b.png)")]
    #[case::uri_autolink("<https://example.com/a...b>")]
    #[case::email_autolink("<first...last@example.com>")]
    #[case::bare_url("https://github.com/org/repo/compare/v1...v2")]
    #[case::relative_path("./fixtures/.../expected.txt")]
    #[case::parent_path("../fixtures/a...b.txt")]
    #[case::absolute_path("/var/lib/.../state")]
    #[case::home_path("~/src/.../README.md")]
    #[case::windows_path(r"C:\src\...\README.md")]
    fn preserves_semantic_dot_runs(#[case] input: &str) {
        assert_eq!(replace_ellipsis(&[input.to_string()]), [input.to_string()]);
    }

    #[test]
    fn preserves_link_reference_destination() {
        let input = vec![
            concat!(
                "[0.1.1]: https://github.com/leynos/diesel-cte-ext/compare/",
                "v0.1.0...302d156361161fd73310926dcef6513b41f7b393",
            )
            .to_string(),
        ];
        assert_eq!(replace_ellipsis(&input), input);
    }

    #[test]
    fn preserves_split_link_reference_destination() {
        let input = vec![
            "[compare]:".to_string(),
            "  https://github.com/leynos/mdtablefix/compare/v1...v2".to_string(),
            "Prose... still changes.".to_string(),
        ];
        let expected = vec![
            "[compare]:".to_string(),
            "  https://github.com/leynos/mdtablefix/compare/v1...v2".to_string(),
            "Prose… still changes.".to_string(),
        ];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn preserves_split_link_reference_title() {
        let input = vec![
            "[compare]:".to_string(),
            "  https://example.com/compare/v1...v2".to_string(),
            "  \"Versions v1...v2\"".to_string(),
            "Prose... still changes.".to_string(),
        ];
        let expected = vec![
            "[compare]:".to_string(),
            "  https://example.com/compare/v1...v2".to_string(),
            "  \"Versions v1...v2\"".to_string(),
            "Prose… still changes.".to_string(),
        ];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn normalizes_slash_delimited_prose() {
        let input = vec!["Choose and/or... input/output...".to_string()];
        let expected = vec!["Choose and/or… input/output…".to_string()];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn normalizes_escaped_autolink() {
        let input = vec![r"\<https://example.com/a...b>".to_string()];
        let expected = vec![r"\<https://example.com/a…b>".to_string()];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[tracing_test::traced_test]
    #[test]
    fn preservation_traces_omit_document_content() {
        let sensitive_line = "    private... payload".to_string();
        let split_reference = vec![
            "[private]:".to_string(),
            "  https://example.com/private...target".to_string(),
        ];

        let _ = replace_ellipsis(std::slice::from_ref(&sensitive_line));
        let _ = replace_ellipsis(&split_reference);

        assert!(logs_contain("reason=\"indented_code\""));
        assert!(logs_contain("reason=\"link_reference_continuation\""));
        assert!(!logs_contain(&sensitive_line));
        assert!(!logs_contain(&split_reference[1]));
    }

    #[test]
    fn replaces_prose_beside_a_literal_url() {
        let input = vec!["Compare... https://example.com/v1...v2".to_string()];
        let expected = vec!["Compare… https://example.com/v1...v2".to_string()];
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
