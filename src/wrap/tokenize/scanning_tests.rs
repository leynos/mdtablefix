//! Unit tests for byte-level tokenizer scanning helpers.

use rstest::rstest;

use super::*;

#[rstest]
#[case::alpha_prefix("abc123", 0, char::is_alphabetic as fn(char) -> bool, 3)]
#[case::numeric_suffix("abc123", 3, char::is_numeric as fn(char) -> bool, "abc123".len())]
#[case::multibyte_scan(
    "åßç123",
    0,
    char::is_alphabetic as fn(char) -> bool,
    "åßç123".find('1').expect("digit should be present in test case")
)]
fn scan_while_cases(
    #[case] text: &str,
    #[case] start: usize,
    #[case] predicate: fn(char) -> bool,
    #[case] expected_idx: usize,
) {
    assert_eq!(scan_while(text, start, predicate), expected_idx);
}

#[rstest]
#[case::first_two("αβγδε", 0, "αβ".len(), "αβ")]
#[case::middle("αβγδε", "αβ".len(), "αβ".len() + "γδ".len(), "γδ")]
fn collect_range_cases(
    #[case] text: &str,
    #[case] start: usize,
    #[case] end: usize,
    #[case] expected: &str,
) {
    assert_eq!(collect_range(text, start, end), expected);
}

#[rstest]
#[case("`VarGuard`s alive", "`VarGuard`".len(), "`VarGuard`s".len())]
#[case("`class`'s field", "`class`".len(), "`class`'s".len())]
#[case("`code`-style name", "`code`".len(), "`code`-style".len())]
#[case("`code`-2 next", "`code`".len(), "`code`".len())]
#[case("`code`.", "`code`".len(), "`code`".len())]
#[case("`code`**", "`code`".len(), "`code`".len())]
#[case("`code`'2 next", "`code`".len(), "`code`".len())]
fn scan_code_suffix_end_cases(#[case] text: &str, #[case] start: usize, #[case] expected: usize) {
    assert_eq!(scan_code_suffix_end(text, start), expected);
}

#[test]
fn parse_open_code_span_returns_active_fence() {
    assert_eq!(parse_open_code_span("`foo"), Some((1, "foo")));
    assert_eq!(parse_open_code_span("text `4.1.1"), Some((1, "4.1.1")));
    assert_eq!(parse_open_code_span("`done` `open"), Some((1, "open")));
    assert_eq!(parse_open_code_span("`done`"), None);
}

#[rstest]
#[case("Version `1.2", "beta` works.", false)]
#[case("Release `4.1.1", "rc1` candidate.", false)]
#[case("text `open", "`close rest", true)]
fn continuation_begins_with_closing_fence_matches_literal_closers_only(
    #[case] existing: &str,
    #[case] continuation: &str,
    #[case] expected: bool,
) {
    assert_eq!(
        continuation_begins_with_closing_fence(existing, continuation),
        expected
    );
}

#[rstest]
#[case("`a``b`", false)]
#[case("``ab``", false)]
#[case("``a`b`", true)]
fn has_unclosed_code_span_rejects_mid_run_closers(#[case] text: &str, #[case] expected: bool) {
    assert_eq!(has_unclosed_code_span(text), expected);
}
