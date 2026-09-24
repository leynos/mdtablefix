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

/// Finds runs long enough to contain at least one prose ellipsis.
///
/// Replacement is performed one run at a time so complete triples become a Unicode ellipsis while
/// a remainder of one or two dots remains literal.
static DOT_RE: LazyLock<Regex> = lazy_regex!(r"\.{3,}", "ellipsis pattern regex should compile");

/// Tracks whether a line belongs to a top-level indented code block.
///
/// This state is deliberately local to ellipsis replacement. Wrapping has its
/// own block classifier, while this pass only needs to decide which original
/// lines must remain byte-for-byte unchanged.
#[derive(Debug)]
struct IndentedCodeTracker {
    /// Whether the previous non-blank line established the current indented code block.
    is_in_block: bool,
    /// Whether the next indented line may start a block after the preceding structure.
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
    /// Classifies one source line and preserves indented-code lines byte-for-byte.
    ///
    /// A paragraph keeps the tracker from treating the next indented line as code, whereas a
    /// completed leaf block permits that transition. Blank lines retain the current block state.
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

    /// Ends the tracked block after a fence or link continuation takes ownership of the line.
    const fn observe_completed_block(&mut self) {
        self.is_in_block = false;
        self.may_start_block = true;
    }
}

/// Reports whether a block leaves no paragraph open before the next source line.
const fn completes_leaf_block(block_kind: Option<BlockKind>) -> bool {
    matches!(
        block_kind,
        Some(
            BlockKind::Heading
                | BlockKind::LinkReferenceDefinition
                | BlockKind::MarkdownlintDirective
        )
    )
}

/// Tokenises one prose line and changes only text tokens, preserving Markdown literals verbatim.
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

/// Replaces ellipses between protected literal spans while copying those spans unchanged.
fn replace_text_ellipsis(text: &str, out: &mut String) {
    let output_start = out.len();
    let mut cursor = 0;
    for span in protected::literal_spans(text) {
        let (Some(prose), Some(literal)) = (text.get(cursor..span.start), text.get(span.clone()))
        else {
            // Roll back this token so malformed protected ranges cannot drop or duplicate text.
            out.truncate(output_start);
            out.push_str(text);
            return;
        };
        replace_dot_runs(prose, out);
        out.push_str(literal);
        cursor = span.end;
    }
    let Some(prose) = text.get(cursor..) else {
        out.truncate(output_start);
        out.push_str(text);
        return;
    };
    replace_dot_runs(prose, out);
}

/// Converts complete dot triples in one text fragment and leaves an incomplete suffix unchanged.
fn replace_dot_runs(text: &str, out: &mut String) {
    if !DOT_RE.is_match(text) {
        out.push_str(text);
        return;
    }

    let replaced = DOT_RE.replace_all(text, |caps: &regex::Captures<'_>| {
        let len = caps[0].len();
        let ellipses = "…".repeat(len.div_euclid(3));
        let leftover = ".".repeat(len.rem_euclid(3));
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
        assert_eq!(
            replace_ellipsis(&["wait...".to_owned()]),
            ["wait…".to_owned()]
        );
    }

    #[test]
    fn ignores_code_spans() {
        let input = ["a `b...` c".to_owned()];
        assert_eq!(replace_ellipsis(&input), input);
    }

    #[test]
    fn ignores_fenced_blocks() {
        let input = ["```".to_owned(), "...".to_owned(), "```".to_owned()];
        assert_eq!(replace_ellipsis(&input), input);
    }

    #[test]
    fn ignores_blockquoted_fenced_blocks() {
        // The depth-aware fence tracker recognizes a fence opened inside a
        // blockquote, so the enclosed `...` stays literal.
        let input = ["> ```".to_owned(), "> ...".to_owned(), "> ```".to_owned()];
        assert_eq!(replace_ellipsis(&input), input);
    }

    #[rstest::rstest]
    #[case::code_block(
        &["Expected output:", "", "    running 2 tests", "    test foo ... ok", "", "    ...", "after..."],
        &["Expected output:", "", "    running 2 tests", "    test foo ... ok", "", "    ...", "after…"]
    )]
    #[case::paragraph_interruption(&["paragraph", "    prose..."], &["paragraph", "    prose…"])]
    fn transforms_indented_lines(#[case] input: &[&str], #[case] expected: &[&str]) {
        let input_lines = input.iter().map(ToString::to_string).collect::<Vec<_>>();
        let expected_lines = expected.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(replace_ellipsis(&input_lines), expected_lines);
    }

    #[rstest::rstest]
    #[case::heading(&["# Heading", "    literal..."])]
    #[case::closed_fence(&["```", "fenced...", "```", "    literal..."])]
    fn completed_blocks_allow_following_indented_code(#[case] input: &[&str]) {
        let input_lines = input.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(replace_ellipsis(&input_lines), input_lines);
    }

    #[rstest::rstest]
    #[case::three_spaces("   ...", "   …")]
    #[case::four_spaces("    ...", "    ...")]
    #[case::one_tab("\t...", "\t...")]
    fn observes_indented_code_boundary(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(replace_ellipsis(&[input.to_owned()]), [expected.to_owned()]);
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
        assert_eq!(replace_ellipsis(&[input.to_owned()]), [input.to_owned()]);
    }

    #[test]
    fn preserves_link_reference_destination() {
        let input = vec![
            concat!(
                "[0.1.1]: https://github.com/leynos/diesel-cte-ext/compare/",
                "v0.1.0...302d156361161fd73310926dcef6513b41f7b393",
            )
            .to_owned(),
        ];
        assert_eq!(replace_ellipsis(&input), input);
    }

    #[test]
    fn preserves_split_link_reference_destination() {
        let input = vec![
            "[compare]:".to_owned(),
            "  https://github.com/leynos/mdtablefix/compare/v1...v2".to_owned(),
            "Prose... still changes.".to_owned(),
        ];
        let expected = vec![
            "[compare]:".to_owned(),
            "  https://github.com/leynos/mdtablefix/compare/v1...v2".to_owned(),
            "Prose… still changes.".to_owned(),
        ];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn preserves_split_link_reference_title() {
        let input = vec![
            "[compare]:".to_owned(),
            "  https://example.com/compare/v1...v2".to_owned(),
            "  \"Versions v1...v2\"".to_owned(),
            "Prose... still changes.".to_owned(),
        ];
        let expected = vec![
            "[compare]:".to_owned(),
            "  https://example.com/compare/v1...v2".to_owned(),
            "  \"Versions v1...v2\"".to_owned(),
            "Prose… still changes.".to_owned(),
        ];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn normalizes_slash_delimited_prose() {
        let input = ["Choose and/or... input/output...".to_owned()];
        let expected = ["Choose and/or… input/output…".to_owned()];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn normalizes_escaped_autolink() {
        let input = vec![r"\<https://example.com/a...b>".to_owned()];
        let expected = vec![r"\<https://example.com/a…b>".to_owned()];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    // Wrapper over `tracing_test::traced_test`; see `test_macros` for why.
    #[test_macros::traced_test]
    #[test]
    fn preservation_traces_omit_document_content() {
        let sensitive_line = "    private... payload".to_owned();
        let split_reference = vec![
            "[private]:".to_owned(),
            "  https://example.com/private...target".to_owned(),
        ];

        drop(replace_ellipsis(std::slice::from_ref(&sensitive_line)));
        drop(replace_ellipsis(&split_reference));

        assert!(logs_contain("reason=\"indented_code\""));
        assert!(logs_contain("reason=\"link_reference_continuation\""));
        assert!(!logs_contain(&sensitive_line));
        assert!(!logs_contain(
            split_reference.get(1).expect("continuation line exists")
        ));
    }

    #[test]
    fn replaces_prose_beside_a_literal_url() {
        let input = vec!["Élan... https://example.com/v1...v2 café...".to_owned()];
        let expected = vec!["Élan… https://example.com/v1...v2 café…".to_owned()];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn replaces_long_sequences() {
        let input = vec![".... ..... ...... .......".to_owned()];
        let expected = vec!["…. ….. …… …….".to_owned()];
        assert_eq!(replace_ellipsis(&input), expected);
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(replace_ellipsis(&[]), Vec::<String>::new());
    }

    #[test]
    fn handles_multiple_fenced_blocks() {
        let input = vec![
            "text...".to_owned(),
            "```".to_owned(),
            "code...".to_owned(),
            "```".to_owned(),
            "more text...".to_owned(),
        ];
        let expected = vec![
            "text…".to_owned(),
            "```".to_owned(),
            "code...".to_owned(),
            "```".to_owned(),
            "more text…".to_owned(),
        ];
        assert_eq!(replace_ellipsis(&input), expected);
    }
}
