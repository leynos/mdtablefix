//! Unit tests for inline-token predicates.

use proptest::prelude::*;
use rstest::rstest;

use super::{
    ends_with_hyphen_prefix,
    is_inline_code_token,
    is_opening_punct,
    is_trailing_punct,
    is_trailing_punctuation_token,
    is_whitespace_token,
    is_year,
    looks_like_bracketed_reference,
    looks_like_footnote_ref,
};

fn backtick_run_strategy() -> BoxedStrategy<String> {
    prop::collection::vec(Just('`'), 1..8)
        .prop_map(|chars| chars.into_iter().collect::<String>())
        .boxed()
}

fn arbitrary_short_string_strategy() -> BoxedStrategy<String> {
    prop::collection::vec(any::<char>(), 0..24)
        .prop_map(|chars| chars.into_iter().collect::<String>())
        .boxed()
}

fn footnote_label_strategy() -> BoxedStrategy<String> {
    prop::string::string_regex("[a-zA-Z0-9_-]+")
        .expect("failed to build footnote label regex strategy")
        .boxed()
}

#[test]
fn is_inline_code_token_rejects_lone_backtick_delimiter() {
    let delimiter = char::from(b'`');
    assert!(!is_inline_code_token(&delimiter.to_string()));
}

#[test]
fn is_inline_code_token_accepts_complete_span() {
    let delimiter = char::from(b'`');
    let token = format!("{delimiter}code{delimiter}");
    assert!(is_inline_code_token(&token));
}

#[test]
fn is_inline_code_token_matches_backtick_delimited_length_rule() {
    proptest!(|(token in backtick_run_strategy())| {
        let expected = token.len() > 1 && token.starts_with('`') && token.ends_with('`');
        prop_assert_eq!(is_inline_code_token(&token), expected);
    });
}

#[test]
fn is_whitespace_token_matches_char_classification() {
    proptest!(|(token in arbitrary_short_string_strategy())| {
        prop_assert_eq!(
            is_whitespace_token(&token),
            token.chars().all(char::is_whitespace)
        );
    });
}

#[test]
fn opening_and_trailing_punct_are_mutually_exclusive_for_ascii_letters() {
    for c in 'a'..='z' {
        assert!(!is_opening_punct(c));
        assert!(!is_trailing_punct(c));
    }
}

#[test]
fn looks_like_footnote_ref_implies_non_empty_label() {
    proptest!(|(label in footnote_label_strategy())| {
        let token = format!("[^{label}]");
        prop_assert!(looks_like_footnote_ref(&token, &mut None));
    });
}

#[test]
fn looks_like_footnote_ref_rejects_empty_label() {
    assert!(!looks_like_footnote_ref("[^]", &mut None));
}

#[rstest]
#[case("pre-", true)]
#[case("LLM-", true)]
#[case("(pre-", true)]
#[case("pré-", true)]
#[case("字-", true)]
#[case("state-of-the-art-", true)]
#[case("-", false)]
#[case("---", false)]
#[case("foo", false)]
#[case("2024-", false)]
fn ends_with_hyphen_prefix_classifies_tokens(#[case] token: &str, #[case] expected: bool) {
    assert_eq!(ends_with_hyphen_prefix(token), expected);
}

#[rstest]
#[case(".", true)]
#[case("!?", true)]
#[case("...", true)]
#[case("", false)]
#[case("abc", false)]
#[case(".x", false)]
fn is_trailing_punctuation_token_classifies_tokens(#[case] token: &str, #[case] expected: bool) {
    assert_eq!(is_trailing_punctuation_token(token), expected);
}

#[rstest]
#[case("2025", true)]
#[case("2025.", true)]
#[case("2025,", true)]
#[case("2008)", true)]
#[case("2008).", true)]
#[case("2008,)", true)]
#[case("999", false)]
#[case("3000", false)]
#[case("2025th.", false)]
#[case(".", false)]
fn is_year_accepts_sentence_trailing_punctuation(#[case] token: &str, #[case] expected: bool) {
    assert_eq!(is_year(token), expected);
}

#[rstest]
#[case("[1]", true)]
#[case("1]", true)]
#[case("[12]", true)]
#[case("[123456]", true)]
#[case("[1],", true)]
#[case("1].", true)]
#[case("[12].", true)]
#[case("[1]]", true)]
#[case("1],]", true)]
#[case("[a]", false)]
#[case("[١٢]", false)]
#[case("[^1]", false)]
#[case("[1](url)", false)]
#[case("[]", false)]
#[case("]", false)]
#[case("[1", false)]
#[case("[1]x", false)]
#[case("[1 2]", false)]
#[case("", false)]
fn looks_like_bracketed_reference_classifies_tokens(#[case] token: &str, #[case] expected: bool) {
    assert_eq!(looks_like_bracketed_reference(token), expected);
}
