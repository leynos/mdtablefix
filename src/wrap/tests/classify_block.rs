//! Block classification wrap tests extracted to keep `tests.rs` below 400 lines.

use rstest::rstest;

use crate::wrap::{BlockKind, LinkReferenceMatcher, classify_block};

#[rstest]
#[case("# Heading", Some(BlockKind::Heading))]
#[case("   # Heading", Some(BlockKind::Heading))]
#[case("    # Heading", None)]
#[case("	# Heading", None)]
#[case("- item", Some(BlockKind::Bullet))]
#[case("1. item", Some(BlockKind::Bullet))]
#[case("> quote", Some(BlockKind::Blockquote))]
#[case("[^1]: footnote", Some(BlockKind::FootnoteDefinition))]
#[case(
    "[ansible]: <https://docs.ansible.com/>",
    Some(BlockKind::LinkReferenceDefinition)
)]
#[case("[foo]:", Some(BlockKind::LinkReferenceDefinition))]
#[case("  [foo]:", Some(BlockKind::LinkReferenceDefinition))]
#[case("    [foo]:", None)]
#[case(
    "<!-- markdownlint-disable -->",
    Some(BlockKind::MarkdownlintDirective)
)]
#[case("2024 revenue", Some(BlockKind::DigitPrefix))]
#[case("a | b", None)]
#[case("plain text", None)]
fn classify_block_detects_markdown_prefixes(
    #[case] line: &str,
    #[case] expected: Option<BlockKind>,
) {
    let matcher = LinkReferenceMatcher::production();
    assert_eq!(classify_block(line, matcher), expected);
}
