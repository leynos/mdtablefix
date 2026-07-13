//! Unit tests for semantic blockquote-prefix parsing.

use rstest::rstest;

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
