//! Token grouping tests for inline segmentation and span determination.

use rstest::rstest;

use super::super::{inline::determine_token_span, tokenize::segment_inline};

#[rstest]
#[case("`code`!", "`code`!")]
#[case("[link](url).", "[link](url).")]
#[case("plain,", "plain,")]
#[case("`code`,", "`code`,")]
#[case("`code`!`more`", "`code`!`more`")]
#[case("`code` `more`", "`code`")]
#[case("`code` `more`,", "`code`")]
#[case("[link](url),", "[link](url),")]
#[case("[link](url)[another](url2)", "[link](url)[another](url2)")]
#[case("[link](url) [another](url2)", "[link](url) [another](url2)")]
#[case("`code` ,", "`code` ,")]
#[case("`code` !", "`code` !")]
#[case("[link](url) .", "[link](url) .")]
#[case("`code!`", "`code!`")]
#[case("[link!](url)", "[link!](url)")]
#[case("(`code`)", "(`code`)")]
#[case("[`code`]", "[`code`]")]
#[case("（`code`）", "（`code`）")]
#[case("「`code`」", "「`code`」")]
#[case("([link](url))", "([link](url))")]
#[case("[[link](url)]", "[[link](url)]")]
fn determine_token_span_groups_related_tokens(#[case] input: &str, #[case] expected_group: &str) {
    let tokens = segment_inline(input);
    let (end, width) = determine_token_span(&tokens, 0);
    let grouped = tokens[..end].join("");
    assert_eq!(grouped, expected_group);
    assert_eq!(width, unicode_width::UnicodeWidthStr::width(expected_group));
}

#[rstest]
#[case("word[link](url)", &["word", "[link](url)"])]
#[case(
    "word[link](url)[another](url2)",
    &["word", "[link](url)", "[another](url2)"]
)]
#[case("word![img](url)", &["word", "![img](url)"])]
fn segment_inline_splits_before_embedded_links(#[case] input: &str, #[case] expected: &[&str]) {
    let tokens = segment_inline(input);
    let actual: Vec<&str> = tokens.iter().map(String::as_str).collect();
    assert_eq!(actual, expected);
}

#[rstest]
#[case(r"\[link](url)")]
#[case(r"word\[link](url)")]
#[case(r"\![img](url)")]
#[case(r"word\![img](url)")]
fn segment_inline_preserves_escaped_link_literals(#[case] input: &str) {
    assert_eq!(segment_inline(input), vec![input.to_string()]);
}
