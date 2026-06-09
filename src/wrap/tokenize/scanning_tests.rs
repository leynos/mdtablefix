//! Unit tests for byte-level tokenizer scanning helpers.

use proptest::prelude::*;
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

/// Verify `position_after_close` handles alternating escaped/literal fences correctly.
#[rstest]
// Single escaped candidate followed by a balanced literal pair: the escaped
// candidate is the real closer for the outer span.
#[case::escaped_then_balanced_literal(
    r"`a\`` b `c`",
    1,   // search_start: after opening `
    1,   // fence_len
    Some(r"`a\``` b ".len()), // byte offset of the first escaped-candidate closer
)]
// Three alternating escaped candidates before a literal that is itself paired:
// the first escaped candidate is the real closer.
#[case::three_escaped_then_paired_literal(
    r"`a\`` x `b\`` y `c\`` z `d` e `f`",
    1,
    1,
    Some(r"`a\``` x ".len()),
)]
// No escaped candidates: the first literal fence is accepted directly.
#[case::no_escaped_candidates("`abc` def", 1, 1, Some(5))]
// Escaped candidate at the end with no subsequent literal: the escaped
// candidate is the fallback.
#[case::only_escaped_candidate(r"`a\`", 1, 1, Some(r"`a\`".len()))]
fn position_after_close_alternating_cases(
    #[case] text: &str,
    #[case] search_start: usize,
    #[case] fence_len: usize,
    #[case] expected: Option<usize>,
) {
    assert_eq!(
        position_after_close(text, search_start, fence_len),
        expected
    );
}

proptest! {
    /// `position_after_close` must always terminate (no infinite loop) and must
    /// never return an offset beyond `text.len()`.
    #[test]
    fn position_after_close_always_terminates_within_bounds(
        text in "[ -~]{0,200}",   // printable ASCII, up to 200 chars
        search_start in 0usize..=200usize,
        fence_len in 1usize..=4usize,
    ) {
        let search_start = search_start.min(text.len());
        let result = position_after_close(&text, search_start, fence_len);
        if let Some(end) = result {
            prop_assert!(
                end <= text.len(),
                "returned offset {end} exceeds text length {}",
                text.len()
            );
            prop_assert!(
                end >= search_start,
                "returned offset {end} precedes search_start {search_start}"
            );
        }
    }

    /// A fence-length of zero must always return `None`.
    #[test]
    fn position_after_close_zero_fence_len_returns_none(
        text in "[ -~]{0,100}",
        search_start in 0usize..=100usize,
    ) {
        let search_start = search_start.min(text.len());
        prop_assert_eq!(position_after_close(&text, search_start, 0), None);
    }
}
