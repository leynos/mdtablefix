//! Unit tests for inline-fragment classification.

use proptest::prelude::*;

use super::*;

proptest! {
    /// has_inline_code_structure must not panic on arbitrary Unicode input.
    #[test]
    fn has_inline_code_structure_never_panics(text in "\\PC*") {
        let _ = has_inline_code_structure(&text);
    }

    /// A string that starts and ends with a matching backtick fence and
    /// contains no embedded fence of the same length must satisfy the predicate.
    #[test]
    fn has_inline_code_structure_detects_simple_fence(
        inner in "[^`]{1,40}",
    ) {
        let text = format!("`{inner}`");
        prop_assert!(has_inline_code_structure(&text));
    }
}

// Fence preceded by an opening bracket is still detected (without_opening path).
proptest! {
    #[test]
    fn has_inline_code_structure_detects_opening_punct_prefix(
        inner in "[^`]{1,40}",
    ) {
        // Opening punctuation trimmed before fence detection.
        let text = format!("(`{inner}`)");
        prop_assert!(has_inline_code_structure(&text));
    }
}

// Fence followed by trailing punctuation is still detected (trimmed path).
proptest! {
    #[test]
    fn has_inline_code_structure_detects_trailing_punct_suffix(
        inner in "[^`]{1,40}",
    ) {
        let text = format!("`{inner}`.");
        prop_assert!(has_inline_code_structure(&text));
    }
}

// Fences longer than one backtick are detected.
proptest! {
    #[test]
    fn has_inline_code_structure_detects_multi_char_fence(
        inner in "[^`]{1,20}",
        fence_len in 2usize..=4usize,
    ) {
        let fence: String = "`".repeat(fence_len);
        let text = format!("{fence}{inner}{fence}");
        prop_assert!(has_inline_code_structure(&text));
    }
}

// Possessive suffix does not prevent detection.
proptest! {
    #[test]
    fn has_inline_code_structure_with_possessive_suffix(
        inner in "[^`]{1,20}",
    ) {
        let text = format!("`{inner}`'s");
        prop_assert!(has_inline_code_structure(&text));
    }
}

// Hyphenated compound suffix does not prevent detection.
proptest! {
    #[test]
    fn has_inline_code_structure_with_hyphen_suffix(
        inner in "[^`]{1,20}",
        word  in "[a-z]{2,10}",
    ) {
        let text = format!("`{inner}`-{word}");
        prop_assert!(has_inline_code_structure(&text));
    }
}

// A Markdown link must NOT be classified as inline code structure.
// This guards against false positives when link text contains backticks.
proptest! {
    #[test]
    fn has_inline_code_structure_does_not_match_plain_link(
        label in "[a-zA-Z]{1,20}",
        url   in "https://[a-z]{3,10}\\.[a-z]{2,4}",
    ) {
        // A plain link with no backticks in the label must not match.
        let text = format!("[{label}]({url})");
        assert!(!has_inline_code_structure(&text));
    }
}
