//! Unit tests for inline Markdown tokenization.

use rstest::rstest;

use super::*;

#[test]
fn segment_inline_handles_multibyte_tokens() {
    let tokens = segment_inline("ßß `λ` фин");
    assert_eq!(
        tokens,
        vec![
            String::from("ßß"),
            String::from(" "),
            String::from("`λ`"),
            String::from(" "),
            String::from("фин"),
        ]
    );
}

#[test]
fn link_with_trailing_punctuation() {
    let tokens = segment_inline("see [link](url).");
    assert_eq!(tokens, vec!["see", " ", "[link](url)", "."]);
}

#[test]
fn image_with_nested_parentheses() {
    let tokens = segment_inline("![alt](path(a(b)c))");
    assert_eq!(tokens, vec!["![alt](path(a(b)c))"]);
}

#[test]
fn inline_code_fences() {
    let tokens = segment_inline("use ``cmd`` now");
    assert_eq!(tokens, vec!["use", " ", "``cmd``", " ", "now"]);
}

#[test]
fn segment_inline_handles_backslash_terminated_code_span() {
    let tokens = segment_inline(r"Install to `C:\path\bin\` and add");
    assert_eq!(
        tokens,
        vec![
            "Install",
            " ",
            "to",
            " ",
            r"`C:\path\bin\`",
            " ",
            "and",
            " ",
            "add"
        ]
    );
}

#[test]
fn unmatched_backticks() {
    let tokens = segment_inline("bad `code span");
    assert_eq!(tokens, vec!["bad", " ", "`", "code", " ", "span"]);
}

#[test]
fn tokenize_marks_trailing_newline() {
    let tokens = tokenize_markdown("foo\n");
    assert_eq!(tokens, vec![Token::Text("foo"), Token::Newline]);
}

#[test]
fn tokenize_handles_crlf() {
    let tokens = tokenize_markdown("foo\r\nbar");
    assert_eq!(
        tokens,
        vec![Token::Text("foo"), Token::Newline, Token::Text("bar")]
    );
}

#[test]
fn segment_inline_splits_escaped_triple_backticks() {
    let tokens = segment_inline(r"\`\`\`ignore");
    assert_eq!(tokens, vec![r"\`", r"\`", r"\`", "ignore"]);
}

#[test]
fn tokenize_markdown_treats_escaped_triple_backticks_as_text() {
    let tokens = tokenize_markdown(r"\`\`\`ignore");
    assert_eq!(tokens, vec![Token::Text(r"\`\`\`ignore")]);
}

#[test]
fn segment_inline_splits_escaped_inline_backtick() {
    let tokens = segment_inline(r"foo\`bar");
    assert_eq!(tokens, vec![r"foo\`", "bar"]);
}

#[test]
fn tokenize_markdown_treats_escaped_inline_backtick_as_text() {
    let tokens = tokenize_markdown(r"foo\`bar");
    assert_eq!(tokens, vec![Token::Text(r"foo\`bar")]);
}

#[test]
fn tokenize_markdown_preserves_nested_blockquote_fences() {
    let source = "> > ```rust\n> > let value = **literal**;\n> > ```";

    assert_eq!(
        tokenize_markdown(source),
        vec![
            Token::Fence("> > ```rust"),
            Token::Newline,
            Token::Fence("> > let value = **literal**;"),
            Token::Newline,
            Token::Fence("> > ```"),
        ]
    );
}

#[rstest]
#[case::german_umlauts(r"ß\`å", vec![r"ß\`", "å"])]
#[case::chinese_characters(r"前\`后", vec![r"前\`", "后"])]
fn segment_inline_splits_escaped_backtick_adjacent_to_multibyte(
    #[case] input: &str,
    #[case] expected: Vec<&str>,
) {
    let tokens = segment_inline(input);
    assert_eq!(tokens, expected);
}

#[rstest]
#[case::german_umlauts(r"ß\`å")]
#[case::chinese_characters(r"前\`后")]
fn tokenize_markdown_treats_escaped_backtick_adjacent_to_multibyte_as_text(#[case] input: &str) {
    let tokens = tokenize_markdown(input);
    assert_eq!(tokens, vec![Token::Text(input)]);
}
