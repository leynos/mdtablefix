//! Proptest property tests for wrapping invariants.

use mdtablefix::wrap::{continuation_begins_with_closing_fence, has_unclosed_code_span, wrap_text};
use proptest::prelude::*;

fn footnote_label_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            (b'a'..=b'z').prop_map(char::from),
            (b'A'..=b'Z').prop_map(char::from),
            (b'0'..=b'9').prop_map(char::from),
            Just('-'),
            Just('_')
        ],
        1..16,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #[test]
    fn wrap_text_keeps_generated_footnote_references_atomic(
        label in footnote_label_strategy(),
        prefix_words in 2usize..30,
        suffix_words in 0usize..=20,
        width in 24usize..96,
    ) {
        let marker = format!("[^{label}]");
        let prefix = std::iter::repeat_n("prefix", prefix_words).collect::<Vec<_>>().join(" ");
        let suffix = std::iter::repeat_n("suffix", suffix_words).collect::<Vec<_>>().join(" ");
        let input = if suffix.is_empty() {
            vec![format!("{prefix} xxxxxxxxxxxxxxxxxxxxx.{marker}")]
        } else {
            vec![format!("{prefix} xxxxxxxxxxxxxxxxxxxxx.{marker} {suffix}")]
        };

        let wrapped = wrap_text(&input, width);
        let rendered = wrapped.join("\n");

        prop_assert!(rendered.contains(&marker));
        prop_assert!(!rendered.contains("[\n"));
        prop_assert!(!rendered.contains("\n^"));
    }

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
    fn wrap_text_never_orphans_closing_backtick(
        content in "[a-z]{1,40}",
        width in 20usize..=100,
    ) {
        let input = vec![format!("- `{content}`")];
        let output = wrap_text(&input, width);
        for line in &output {
            let stripped = line.strip_prefix("  ").unwrap_or(line);
            if stripped.starts_with('`') {
                prop_assert!(
                    stripped.ends_with('`'),
                    "orphaned closing backtick fragment on line: {line:?}"
                );
            }
        }
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
        let close_fence = "`".repeat(n + delta);
        let existing = format!("{open_fence}{prefix}");
        let continuation = format!("{close_fence}{suffix}");
        prop_assert!(
            !continuation_begins_with_closing_fence(&existing, &continuation)
        );
    }
}
