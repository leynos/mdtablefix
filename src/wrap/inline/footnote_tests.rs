//! Regression tests for inline GFM footnote-reference wrapping.

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
