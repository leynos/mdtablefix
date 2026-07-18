//! Tests for the `FenceTracker` helper.
//!
//! These cases exercise fence detection across various markers and spacing so
//! the wrapper skips reflow inside fenced code blocks.

use rstest::rstest;

use crate::wrap::{FenceTracker, is_fence};

#[test]
fn fence_tracker_new_starts_outside_fence() {
    let tracker = FenceTracker::new();
    assert!(!tracker.in_fence(0));
}

#[test]
fn fence_tracker_closes_matching_markers() {
    let mut tracker = FenceTracker::default();
    assert!(!tracker.in_fence(0));
    assert!(tracker.observe("```rust", 0));
    assert!(tracker.in_fence(0));
    assert!(tracker.observe("```", 0));
    assert!(!tracker.in_fence(0));
}

#[test]
fn fence_tracker_closes_with_info_string() {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe("```rust", 0));
    assert!(tracker.in_fence(0));
    assert!(tracker.observe("```   ", 0));
    assert!(!tracker.in_fence(0));
}

#[test]
fn fence_tracker_ignores_shorter_closing_marker() {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe("````", 0));
    assert!(tracker.in_fence(0));
    assert!(tracker.observe("```", 0));
    assert!(tracker.in_fence(0));
}

#[test]
fn fence_tracker_requires_matching_marker_to_close() {
    let mut tracker = FenceTracker::default();
    assert!(tracker.observe("```", 0));
    assert!(tracker.in_fence(0));
    assert!(tracker.observe("~~~", 0));
    assert!(tracker.in_fence(0));
    assert!(tracker.observe("````", 0));
    assert!(!tracker.in_fence(0));
}

#[test]
fn fence_tracker_handles_inline_and_indented_markers() {
    let lines = [
        "```rust code fence on one line```",
        "   ```   ",
        "text outside fence",
        "```",
        concat!(
            "text inside fence that should remain intact even if it exceeds the usual width ",
            "limit when wrapping is enabled."
        ),
        "```   ",
        "text after fence",
    ];
    let mut tracker = FenceTracker::default();
    let results: Vec<bool> = lines.iter().map(|line| tracker.observe(line, 0)).collect();
    assert_eq!(
        results,
        vec![true, true, false, true, false, true, false],
        "expected fences to be recognised with inline markers and atypical spacing"
    );
    assert!(
        !tracker.in_fence(0),
        "tracker should end outside of a fence after matching closures"
    );
}

#[test]
fn fence_tracker_handles_tilde_fences() {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe("~~~~rust", 0));
    assert!(tracker.in_fence(0));
    assert!(tracker.observe("~~~~", 0));
    assert!(!tracker.in_fence(0));
}

#[rstest]
#[case("````markdown", "```rust", "```", "````", false)]
#[case("````", "~~~", "~~~", "````", false)]
#[case("~~~~", "```", "```", "~~~~", false)]
#[case("~~~~markdown", "~~~rust", "~~~", "~~~~", false)]
fn fence_tracker_keeps_outer_fence_open_for_nested_markers(
    #[case] outer_start: &str,
    #[case] inner_start: &str,
    #[case] inner_end: &str,
    #[case] outer_end: &str,
    #[case] expected_final_in_fence: bool,
) {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe(outer_start, 0));
    assert!(tracker.in_fence(0));
    assert!(tracker.observe(inner_start, 0));
    assert!(tracker.in_fence(0));
    assert!(tracker.observe(inner_end, 0));
    assert!(tracker.in_fence(0));
    assert!(tracker.observe(outer_end, 0));
    assert_eq!(tracker.in_fence(0), expected_final_in_fence);
}

#[rstest]
#[case("`")]
#[case("``")]
#[case("`~~`")]
#[case("~~`")]
#[case("`` ~~")]
fn fence_tracker_rejects_short_or_mixed_markers(#[case] line: &str) {
    let mut tracker = FenceTracker::default();
    assert!(!tracker.observe(line, 0));
    assert!(!tracker.in_fence(0));
}

#[test]
fn fence_tracker_opens_and_closes_at_nested_depth() {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe("```rust", 2));
    assert!(tracker.in_fence(2));
    assert!(tracker.observe("```", 2));
    assert!(!tracker.in_fence(2));
}

#[test]
fn fence_tracker_closes_when_blockquote_depth_decreases() {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe("```rust", 2));
    assert!(!tracker.observe("plain text", 1));
    assert!(!tracker.in_fence(1));
}

#[test]
fn fence_tracker_remains_open_for_deeper_content() {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe("```rust", 1));
    assert!(!tracker.observe("plain text", 2));
    assert!(tracker.in_fence(2));
}

#[rstest]
#[case("> ```rust", "> ", 1)]
#[case("> > ~~~~toml", "> > ", 2)]
#[case(">>```", ">>", 2)]
fn raw_blockquote_fences_preserve_prefix_and_depth(
    #[case] opening: &str,
    #[case] expected_prefix: &str,
    #[case] depth: usize,
) {
    let (prefix, _marker, _info) = is_fence(opening).expect("quoted fence should be recognized");
    assert_eq!(prefix, expected_prefix);

    let mut tracker = FenceTracker::new();
    assert!(tracker.observe_line(opening));
    assert!(tracker.in_fence_for_line(opening));
    assert!(tracker.in_fence(depth));
}

#[test]
fn raw_blockquote_fence_closes_when_quote_depth_decreases() {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe_line("> > ```rust"));
    assert!(!tracker.observe_line("> ordinary quote text"));
    assert!(!tracker.in_fence_for_line("> ordinary quote text"));
}
