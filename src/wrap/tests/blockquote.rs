//! Unit tests for semantic blockquote-prefix parsing.

use rstest::rstest;
use tracing_test::traced_test;

use crate::wrap::BlockquotePrefix;

#[rstest]
#[case::single_level("> quoted", "> ", 1, "quoted")]
#[case::nested_with_spaces("> > quoted", "> > ", 2, "quoted")]
#[case::compact_nested(">>quoted", ">>", 2, "quoted")]
#[case::mixed_tabs("> \t> quoted", "> \t> ", 2, "quoted")]
fn parses_blockquote_prefixes(
    #[case] line: &str,
    #[case] expected_raw_prefix: &str,
    #[case] expected_depth: usize,
    #[case] expected_inner: &str,
) {
    let prefix = BlockquotePrefix::parse(line).expect("the blockquote prefix should parse");

    assert_eq!(prefix.raw_prefix(), expected_raw_prefix);
    assert_eq!(prefix.depth(), expected_depth);
    assert_eq!(prefix.inner(), expected_inner);
}

#[rstest]
#[case::plain_text("plain text")]
#[case::list_item("- list item")]
#[case::fence("```rust")]
fn rejects_non_blockquote_lines(#[case] line: &str) {
    assert_eq!(BlockquotePrefix::parse(line), None);
}

#[traced_test]
#[test]
fn successful_parse_logs_content_free_dimensions() {
    let input = "> private blockquote payload";
    let parsed = BlockquotePrefix::parse(input);

    assert!(parsed.is_some());
    assert!(logs_contain("blockquote prefix parsed"));
    assert!(logs_contain(&format!("line_len={}", input.len())));
    assert!(logs_contain("prefix_len=2"));
    assert!(logs_contain("depth=1"));
    assert!(logs_contain("inner_len=26"));
    assert!(!logs_contain(input));
    assert!(!logs_contain("private blockquote payload"));
}

#[traced_test]
#[test]
fn rejected_parse_logs_content_free_reason() {
    let input = "private ordinary payload";
    let parsed = BlockquotePrefix::parse(input);

    assert!(parsed.is_none());
    assert!(logs_contain("blockquote prefix rejected"));
    assert!(logs_contain("reason=\"no_blockquote_prefix\""));
    assert!(logs_contain(&format!("line_len={}", input.len())));
    assert!(!logs_contain(input));
}
