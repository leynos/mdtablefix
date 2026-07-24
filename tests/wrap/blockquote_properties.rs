//! Property tests for depth-aware blockquote wrapping and fenced code state.

use mdtablefix::wrap::{BlockquotePrefix, FenceTracker, wrap_text};
use proptest::prelude::*;
use unicode_width::UnicodeWidthStr;

fn blockquote_word_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z]{1,12}".prop_map(String::from),
        prop::sample::select(vec!["café", "naïve", "漢字", "🙂", "🌍"]).prop_map(String::from),
    ]
}

proptest! {
    #[test]
    fn fences_close_only_at_their_open_depth_or_above_container_exit(
        depth in 1_usize..=4,
        marker in prop_oneof![Just('`'), Just('~')],
        marker_len in 3_usize..=8,
        nested_depth_offset in 1_usize..=3,
        inner in "[a-zA-Z0-9 _-]{0,40}",
    ) {
        let fence = marker.to_string().repeat(marker_len);
        let opening_fence = format!("{fence}rust");
        let mut tracker = FenceTracker::new();

        prop_assert!(tracker.observe(&opening_fence, depth));
        prop_assert!(!tracker.observe(&inner, depth + nested_depth_offset));
        prop_assert!(tracker.in_fence(depth + nested_depth_offset));
        for offset in 1..=nested_depth_offset {
            prop_assert!(tracker.observe(&fence, depth + offset));
            prop_assert!(tracker.in_fence(depth + offset));
        }
        prop_assert!(tracker.observe(&fence, depth));
        prop_assert!(!tracker.in_fence(depth));

        prop_assert!(tracker.observe(&fence, depth));
        prop_assert!(!tracker.observe(&inner, depth - 1));
        prop_assert!(!tracker.in_fence(depth));
    }

    #[test]
    fn wrapping_preserves_blockquote_content_exactly_once(
        depth in 1_usize..=4,
        compact in any::<bool>(),
        words in prop::collection::vec(blockquote_word_strategy(), 1..30),
    ) {
        let prefix = if compact {
            ">".repeat(depth)
        } else {
            "> ".repeat(depth)
        };
        let inner = words.join(" ");
        let input = vec![format!("{prefix}{inner}")];

        let output = wrap_text(&input, 32);
        let mut emitted_words = Vec::new();
        for line in &output {
            let parsed = BlockquotePrefix::parse(line)
                .expect("wrapped blockquote output should retain its prefix");
            prop_assert_eq!(parsed.raw_prefix(), prefix.as_str());
            prop_assert_eq!(parsed.depth(), depth);
            prop_assert!(UnicodeWidthStr::width(line.as_str()) <= 32);
            emitted_words.extend(parsed.inner().split_whitespace());
        }

        prop_assert_eq!(emitted_words, words);
    }
}
