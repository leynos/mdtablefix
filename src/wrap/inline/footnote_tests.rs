//! Regression tests for inline GFM footnote-reference wrapping.
//!
//! This module exercises the inline tokenisation and span-selection logic
//! used by the wrapping code when Markdown footnote references appear in
//! running text. It keeps footnote references atomic, verifies that adjacent
//! punctuation stays with the reference when appropriate, and protects the
//! segmenter from splitting references into invalid wrap points.

use rstest::rstest;

use super::determine_token_span;
use crate::wrap::tokenize::segment_inline;

#[test]
fn determine_token_span_groups_punctuation_with_footnote_reference() {
    let tokens = segment_inline("word.[^4]");
    let (end, width) = determine_token_span(&tokens, 0);
    let grouped = tokens[..end].join("");
    assert_eq!(grouped, "word.[^4]");
    assert_eq!(width, unicode_width::UnicodeWidthStr::width("word.[^4]"));
}

#[test]
fn determine_token_span_groups_definition_like_footnote_reference_with_prose() {
    let input = "subcategories [^96]:";
    let tokens = segment_inline(input);
    let (end, width) = determine_token_span(&tokens, 0);
    let grouped = tokens[..end].join("");

    assert_eq!(grouped, input);
    assert_eq!(width, unicode_width::UnicodeWidthStr::width(input));
}

#[rstest]
#[case("`fn!()`.[^1]")]
#[case("`value`,[^2]")]
#[case("`result`?[^3]")]
#[case("[text](url).[^1]")]
#[case("[text](url),[^2]")]
#[case("(`code`).[^1]")]
#[case("([link](url)).[^1]")]
#[case("`assert_ne!`.[^3]")]
#[case("[click here](https://example.com).[^1]")]
fn determine_token_span_groups_footnote_reference_after_atomic_span(#[case] input: &str) {
    let tokens = segment_inline(input);
    let (end, width) = determine_token_span(&tokens, 0);
    let grouped = tokens[..end].join("");
    assert_eq!(grouped, input);
    assert_eq!(width, unicode_width::UnicodeWidthStr::width(input));
}

#[rstest]
#[case("`assert_ne!`. [^3]", "`assert_ne!`.")]
#[case("`assert_ne!` [^3]", "`assert_ne!`")]
fn determine_token_span_does_not_group_whitespace_separated_footnote_reference(
    #[case] input: &str,
    #[case] expected_group: &str,
) {
    let tokens = segment_inline(input);
    let (end, width) = determine_token_span(&tokens, 0);
    let grouped = tokens[..end].join("");
    assert_eq!(grouped, expected_group);
    assert_eq!(width, unicode_width::UnicodeWidthStr::width(expected_group));
}

#[test]
fn determine_token_span_groups_footnote_reference_after_opener_coupled_code() {
    let input = "See (`code`).[^1] for details";
    let tokens = segment_inline(input);
    let open_paren = tokens
        .iter()
        .position(|token| token == "(")
        .expect("opening parenthesis token");
    let (end, width) = determine_token_span(&tokens, open_paren);
    let grouped = tokens[open_paren..end].join("");
    let expected = "(`code`).[^1]";
    assert_eq!(grouped, expected);
    assert_eq!(width, unicode_width::UnicodeWidthStr::width(expected));
}

#[test]
fn segment_inline_splits_before_embedded_footnote_reference() {
    let tokens = segment_inline("word[^4]");
    let actual: Vec<&str> = tokens.iter().map(String::as_str).collect();
    assert_eq!(actual, ["word", "[^4]"]);
}

#[rstest]
#[case("see [^4] now", &["see", " ", "[^4]", " ", "now"])]
#[case("see [^25] now", &["see", " ", "[^25]", " ", "now"])]
#[case("see [^note] now", &["see", " ", "[^note]", " ", "now"])]
fn segment_inline_preserves_footnote_references(#[case] input: &str, #[case] expected: &[&str]) {
    let tokens = segment_inline(input);
    let actual: Vec<&str> = tokens.iter().map(String::as_str).collect();
    assert_eq!(actual, expected);
}
