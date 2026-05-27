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
fn determine_token_span_groups_footnote_reference_after_inline_code() {
    let input = "`assert_ne!`.[^3]";
    let tokens = segment_inline(input);
    let (end, width) = determine_token_span(&tokens, 0);
    let grouped = tokens[..end].join("");
    assert_eq!(grouped, input);
    assert_eq!(width, unicode_width::UnicodeWidthStr::width(input));
}

#[test]
fn determine_token_span_groups_footnote_reference_after_link() {
    let input = "[click here](https://example.com).[^1]";
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
fn determine_token_span_groups_footnote_reference_after_atomic_span(#[case] input: &str) {
    let tokens = segment_inline(input);
    let (end, width) = determine_token_span(&tokens, 0);
    let grouped = tokens[..end].join("");
    assert_eq!(grouped, input);
    assert_eq!(width, unicode_width::UnicodeWidthStr::width(input));
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
