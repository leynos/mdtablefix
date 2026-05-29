//! Unit tests for code span state helpers.
//!
//! These tests call `pub(crate)` items directly and must live inside the
//! library crate rather than in the integration-test directory.

use proptest::prelude::*;

use super::super::{continuation_begins_with_closing_fence, has_unclosed_code_span};
use crate::wrap::tokenize::{
    opening_fence_run_len,
    parse_open_code_span,
    scan_continuation_span_state,
};

proptest! {
    #[test]
    fn has_unclosed_code_span_is_false_for_closed_span(
        n in 1usize..=3,
        content in "[^`]+",
    ) {
        // Reject content that ends with a backslash so the closing
        // backtick is not escaped.
        prop_assume!(!content.ends_with('\\'));
        let fence = "`".repeat(n);
        let text = format!("{fence}{content}{fence}");
        prop_assert!(!has_unclosed_code_span(&text));
    }

    #[test]
    fn has_unclosed_code_span_is_true_when_closer_absent(
        n in 1usize..=3,
        content in "[^`]+",
    ) {
        let fence = "`".repeat(n);
        let text = format!("{fence}{content}");
        prop_assert!(has_unclosed_code_span(&text));
    }

    #[test]
    fn continuation_begins_with_closing_fence_accepts_exact_match(
        n in 1usize..=3,
        prefix in "[^`]*",
        suffix in "[^`]*",
    ) {
        let fence = "`".repeat(n);
        let existing = format!("{fence}{prefix}");
        let continuation = format!("{fence}{suffix}");
        prop_assert!(
            continuation_begins_with_closing_fence(&existing, &continuation)
        );
    }

    #[test]
    fn continuation_begins_with_closing_fence_rejects_length_mismatch(
        n in 1usize..=3,
        delta in 1usize..=3,
        prefix in "[^`]*",
        suffix in "[^`]*",
    ) {
        let open_fence = "`".repeat(n);
        let existing = format!("{open_fence}{prefix}");

        let close_long = "`".repeat(n + delta);
        let continuation_long = format!("{close_long}{suffix}");
        prop_assert!(
            !continuation_begins_with_closing_fence(&existing, &continuation_long),
            "longer closing fence must not match"
        );

        if n > delta {
            let close_short = "`".repeat(n - delta);
            let continuation_short = format!("{close_short}{suffix}");
            prop_assert!(
                !continuation_begins_with_closing_fence(&existing, &continuation_short),
                "shorter closing fence must not match"
            );
        }
    }

    #[test]
    fn opening_fence_run_len_detects_unescaped_backtick_run(
        n in 1usize..=4,
        suffix in "[^`]*",
    ) {
        let fence = "`".repeat(n);
        let text = format!("{fence}{suffix}");
        let bytes = text.as_bytes();
        let result = opening_fence_run_len(bytes, &text);
        prop_assert_eq!(result, Some(n));
    }

    #[test]
    fn parse_open_code_span_rejects_escaped_backtick(suffix in "[^`]*") {
        // A backslash escapes only the immediately following character, so
        // we limit the fence to a single backtick: "\\`{suffix}" has no
        // unescaped backticks at all. `parse_open_code_span` walks each
        // position and consults `has_odd_backslash_escape_bytes`, so it
        // actually exercises escape handling — unlike
        // `opening_fence_run_len`, which never sees the backtick because
        // the leading backslash sits at byte 0 and short-circuits the
        // first-character check.
        let text = format!("\\`{suffix}");
        prop_assert!(
            parse_open_code_span(&text).is_none(),
            "escaped backtick must not be detected as opener; text={text:?}"
        );
    }

    #[test]
    fn scan_continuation_span_state_none_when_balanced(
        n in 1usize..=3,
        content in "[^`]+",
    ) {
        // Reject content ending with backslash so the closing fence is not
        // escaped.
        prop_assume!(!content.ends_with('\\'));
        let fence = "`".repeat(n);
        let continuation = format!("{fence}{content}");
        let result = scan_continuation_span_state(&continuation, n);
        prop_assert!(
            result.is_none(),
            "span of length {n} should close at start of continuation; \
             continuation={continuation:?} result={result:?}"
        );
    }

    #[test]
    fn scan_continuation_span_state_some_when_no_closer(
        n in 1usize..=3,
        content in "[^`]+",
    ) {
        // Continuation has no matching closing fence; span should remain open.
        let fence_wrong_len = "`".repeat(n + 1);
        let continuation = format!("{fence_wrong_len}{content}");
        let result = scan_continuation_span_state(&continuation, n);
        prop_assert!(
            result.is_some(),
            "span of length {n} should stay open when no matching closer; \
             continuation={continuation:?} result={result:?}"
        );
    }
}

#[rstest::rstest]
#[case("`foo`", false)]
#[case("`foo", true)]
#[case("`` foo ``", false)]
#[case("`` foo `", true)]
#[case(r"\`foo", false)]
#[case("`done` `open", true)]
fn has_unclosed_code_span_detects_open_fences(#[case] text: &str, #[case] expected: bool) {
    assert_eq!(has_unclosed_code_span(text), expected);
}
