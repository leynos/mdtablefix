//! Unit tests for text wrapping functionality.
//!
//! This module contains tests for the `wrap_text` function, verifying correct
//! behaviour with code spans, links, hyphenated words, and various line widths.

use rstest::rstest;

use super::{
    inline::{
        attach_punctuation_to_previous_line,
        determine_token_span,
        wrap_preserving_code,
    },
    line_buffer::LineBuffer,
    tokenize::segment_inline,
};
use crate::wrap::{BlockKind, classify_block, wrap_text};

#[rstest]
#[case("`code`!", "`code`!")]
#[case("[link](url).", "[link](url).")]
#[case("plain,", "plain,")]
#[case("`code`,", "`code`,")]
#[case("`code`!`more`", "`code`!`more`")]
#[case("[link](url),", "[link](url),")]
#[case("[link](url)[another](url2)", "[link](url)[another](url2)")]
#[case("[link](url) [another](url2)", "[link](url) [another](url2)")]
#[case("`code` ,", "`code` ,")]
#[case("`code` !", "`code` !")]
#[case("[link](url) .", "[link](url) .")]
#[case("`code!`", "`code!`")]
#[case("[link!](url)", "[link!](url)")]
fn determine_token_span_groups_related_tokens(#[case] input: &str, #[case] expected_group: &str) {
    let tokens = segment_inline(input);
    let (end, width) = determine_token_span(&tokens, 0);
    let grouped = tokens[..end].join("");
    assert_eq!(grouped, expected_group);
    assert_eq!(width, unicode_width::UnicodeWidthStr::width(expected_group));
}

#[test]
fn attach_punctuation_appends_to_previous_code_line() {
    let mut lines = vec!["wrap `code`".to_string()];
    let current = String::new();
    assert!(attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        "!",
    ));
    assert_eq!(lines, vec!["wrap `code`!".to_string()]);
}

#[test]
fn attach_punctuation_requires_empty_current_buffer() {
    let mut lines = vec!["`code`".to_string()];
    let current = " pending".to_string();
    assert!(!attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        "!",
    ));
    assert_eq!(lines, vec!["`code`".to_string()]);
}

#[test]
fn attach_punctuation_ignores_non_code_suffix() {
    let mut lines = vec!["plain text".to_string()];
    let current = String::new();
    assert!(!attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        ".",
    ));
    assert_eq!(lines, vec!["plain text".to_string()]);
}

#[test]
fn line_buffer_trims_trailing_whitespace_before_punctuation() {
    let tokens = vec![
        "wrap".to_string(),
        " ".to_string(),
        "`code`".to_string(),
        "  ".to_string(),
    ];
    let mut buffer = LineBuffer::new();
    buffer.push_span(&tokens, 0, tokens.len());
    assert_eq!(buffer.text(), "wrap `code`  ");

    let punct = vec!["!".to_string()];
    buffer.push_span(&punct, 0, punct.len());
    assert_eq!(buffer.text(), "wrap `code`!");
}

#[test]
fn line_buffer_split_preserves_multi_space_lines() {
    let tokens = vec![
        "alpha".to_string(),
        "  ".to_string(),
        "beta".to_string(),
        "   ".to_string(),
    ];
    let mut buffer = LineBuffer::new();
    buffer.push_span(&tokens, 0, 2);

    let mut lines = Vec::new();
    assert!(buffer.split_with_span(&mut lines, &tokens, 2, 4, 8));
    assert_eq!(lines, vec!["alpha  ".to_string()]);
    assert_eq!(buffer.text(), "beta   ");
    assert_eq!(
        buffer.width(),
        unicode_width::UnicodeWidthStr::width(buffer.text())
    );
}

#[test]
fn line_buffer_split_trims_single_trailing_space() {
    let tokens = vec!["alpha".to_string(), " ".to_string(), "beta".to_string()];
    let mut buffer = LineBuffer::new();
    buffer.push_span(&tokens, 0, 2);

    let mut lines = Vec::new();
    assert!(buffer.split_with_span(&mut lines, &tokens, 2, 3, 5));
    assert_eq!(lines, vec!["alpha".to_string()]);
    assert_eq!(buffer.text(), "beta");
    assert_eq!(
        buffer.width(),
        unicode_width::UnicodeWidthStr::width(buffer.text())
    );
}

#[test]
fn line_buffer_split_tracks_multiple_whitespace_tokens() {
    let tokens = vec![
        "foo".to_string(),
        " ".to_string(),
        " ".to_string(),
        "bar".to_string(),
    ];
    let mut buffer = LineBuffer::new();
    buffer.push_span(&tokens, 0, 3);

    let mut lines = Vec::new();
    assert!(buffer.split_with_span(&mut lines, &tokens, 3, 4, 4));
    assert_eq!(lines, vec!["foo  ".to_string()]);
    assert_eq!(buffer.text(), "bar");
}

#[test]
fn line_buffer_trailing_whitespace_flushes_line() {
    let mut buffer = LineBuffer::new();
    let words = vec!["foo".to_string()];
    buffer.push_span(&words, 0, words.len());

    let whitespace_tokens = vec!["  ".to_string()];
    let mut lines = Vec::new();
    assert!(buffer.flush_trailing_whitespace(
        &mut lines,
        &whitespace_tokens,
        0,
        whitespace_tokens.len(),
    ));
    assert_eq!(lines, vec!["foo  ".to_string()]);
    assert!(buffer.text().is_empty());
}

#[test]
fn wrap_preserving_code_splits_after_consecutive_whitespace() {
    let lines = wrap_preserving_code("alpha  beta   gamma", 8);
    assert_eq!(
        lines,
        vec![
            "alpha  ".to_string(),
            "beta   ".to_string(),
            "gamma".to_string()
        ]
    );
}

#[test]
fn wrap_preserving_code_glues_punctuation_after_code() {
    let lines = wrap_preserving_code("line with `code` !", 80);
    assert_eq!(lines, vec!["line with `code`!".to_string()]);
}

#[test]
fn wrap_text_preserves_hyphenated_words() {
    let input = vec!["A word that is very-long-word indeed".to_string()];
    let wrapped = wrap_text(&input, 20);
    assert_eq!(
        wrapped,
        vec![
            "A word that is".to_string(),
            "very-long-word".to_string(),
            "indeed".to_string(),
        ]
    );
}

#[test]
fn wrap_text_does_not_insert_spaces_in_hyphenated_words() {
    let input = vec![
        concat!(
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Donec tincidunt ",
            "elit-sed fermentum congue. Vivamus dictum nulla sed consectetur ",
            "volutpat.",
        )
        .to_string(),
    ];
    let wrapped = wrap_text(&input, 80);
    assert_eq!(
        wrapped,
        vec![
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Donec tincidunt".to_string(),
            "elit-sed fermentum congue. Vivamus dictum nulla sed consectetur volutpat.".to_string(),
        ]
    );
}

#[test]
fn wrap_text_preserves_code_spans() {
    let input = vec![
        "with their own escaping rules. On Windows, scripts default to `powershell -Command` \
         unless the manifest's `interpreter` field overrides the setting."
            .to_string(),
    ];
    let wrapped = wrap_text(&input, 60);
    assert_eq!(
        wrapped,
        vec![
            "with their own escaping rules. On Windows, scripts default".to_string(),
            "to `powershell -Command` unless the manifest's".to_string(),
            "`interpreter` field overrides the setting.".to_string(),
        ]
    );
}

#[test]
fn wrap_text_multiple_code_spans() {
    let input = vec!["combine `foo bar` and `baz qux` in one line".to_string()];
    let wrapped = wrap_text(&input, 25);
    assert_eq!(
        wrapped,
        vec![
            "combine `foo bar` and".to_string(),
            "`baz qux` in one line".to_string(),
        ]
    );
}

#[test]
fn wrap_text_nested_backticks() {
    let input = vec!["Use `` `code` `` to quote backticks".to_string()];
    let wrapped = wrap_text(&input, 20);
    assert_eq!(
        wrapped,
        vec![
            "Use `` `code` `` to".to_string(),
            "quote backticks".to_string()
        ],
    );
}

#[test]
fn wrap_text_unmatched_backticks() {
    let input = vec!["This has a `dangling code span.".to_string()];
    let wrapped = wrap_text(&input, 20);
    assert_eq!(
        wrapped,
        vec!["This has a".to_string(), "`dangling code span.".to_string()],
    );
}

#[test]
fn wrap_text_preserves_links() {
    let input = vec![
        "`falcon-pachinko` is an extension library for the".to_string(),
        "[Falcon](https://falcon.readthedocs.io) web framework. It adds a structured".to_string(),
        "approach to asynchronous WebSocket routing and background worker integration.".to_string(),
    ];
    let wrapped = wrap_text(&input, 80);
    let joined = wrapped.join("\n");
    assert_eq!(joined.matches("https://").count(), 1);
    assert!(
        wrapped
            .iter()
            .any(|l| l.contains("https://falcon.readthedocs.io"))
    );
}

#[rstest]
#[case("trail  ", 80, &["trail  "])]
#[case("`code span`  ", 12, &["`code span`  "])]
#[case("foo  ", 3, &["foo  "])]
#[case("x  ", 1, &["x  "])]
fn preserves_trailing_spaces(#[case] input: &str, #[case] width: usize, #[case] expected: &[&str]) {
    let out = wrap_preserving_code(input, width);
    assert_eq!(
        out,
        expected.iter().map(|&s| s.to_string()).collect::<Vec<_>>()
    );
}

#[rstest]
#[case("aaaaaaaaaaaa", 5, &["aaaaaaaaaaaa"])] // forced flush without split
#[case("abcde", 3, &["abcde"])]
#[case("`codespan`", 6, &["`codespan`"])]
fn no_split_forced_flush_no_trim(
    #[case] input: &str,
    #[case] width: usize,
    #[case] expected: &[&str],
) {
    let out = wrap_preserving_code(input, width);
    assert_eq!(
        out,
        expected.iter().map(|&s| s.to_string()).collect::<Vec<_>>()
    );
}

#[test]
fn wrap_text_keeps_trailing_spaces_for_blockquote_final_line() {
    // "> " is the prefix; available width = 10 - 2 = 8.
    let input = vec!["> word1 word2  ".to_string()];
    let wrapped = wrap_text(&input, 10);
    assert_eq!(
        wrapped,
        vec!["> word1".to_string(), "> word2  ".to_string()]
    );
}

#[test]
fn wrap_text_keeps_trailing_spaces_for_bullet_final_line() {
    // "- " is the prefix; continuation lines are indented with two spaces.
    let input = vec!["- word1 word2  ".to_string()];
    let wrapped = wrap_text(&input, 10);
    assert_eq!(
        wrapped,
        vec!["- word1".to_string(), "  word2  ".to_string()]
    );
}

#[test]
fn wrap_text_preserves_indented_hash_as_text() {
    let input = vec![
        "Paragraph intro.".to_string(),
        "    # code".to_string(),
        "Continuation.".to_string(),
    ];
    let wrapped = wrap_text(&input, 40);
    assert_eq!(
        wrapped,
        vec![
            "Paragraph intro.".to_string(),
            "    # code".to_string(),
            "Continuation.".to_string(),
        ]
    );
}

#[rstest(
    line,
    expected,
    case("# Heading", Some(BlockKind::Heading)),
    case("   # Heading", Some(BlockKind::Heading)),
    case("    # Heading", None),
    case("- item", Some(BlockKind::Bullet)),
    case("1. item", Some(BlockKind::Bullet)),
    case("> quote", Some(BlockKind::Blockquote)),
    case("[^1]: footnote", Some(BlockKind::FootnoteDefinition)),
    case(
        "<!-- markdownlint-disable -->",
        Some(BlockKind::MarkdownlintDirective)
    ),
    case("2024 revenue", Some(BlockKind::DigitPrefix)),
    case("a | b", None),
    case("plain text", None)
)]
fn classify_block_detects_markdown_prefixes(line: &str, expected: Option<BlockKind>) {
    assert_eq!(classify_block(line), expected);
}

mod fence_tracker;
