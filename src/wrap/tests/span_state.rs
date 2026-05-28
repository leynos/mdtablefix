//! Unit tests for code span state helpers.
//!
//! These tests call `pub(crate)` items directly and must live inside the
//! library crate rather than in the integration-test directory.

use proptest::prelude::*;

use super::super::{continuation_begins_with_closing_fence, has_unclosed_code_span};

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
