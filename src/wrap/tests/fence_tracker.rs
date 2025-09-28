//! Tests for the `FenceTracker` helper.
//!
//! These cases exercise fence detection across various markers and spacing so
//! the wrapper skips reflow inside fenced code blocks.

use rstest::rstest;

use crate::wrap::FenceTracker;

#[test]
fn fence_tracker_new_starts_outside_fence() {
    let tracker = FenceTracker::new();
    assert!(!tracker.in_fence());
}

#[test]
fn fence_tracker_closes_matching_markers() {
    let mut tracker = FenceTracker::default();
    assert!(!tracker.in_fence());
    assert!(tracker.observe("```rust"));
    assert!(tracker.in_fence());
    assert!(tracker.observe("```"));
    assert!(!tracker.in_fence());
}

#[test]
fn fence_tracker_closes_with_info_string() {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe("```rust"));
    assert!(tracker.in_fence());
    assert!(tracker.observe("```   "));
    assert!(!tracker.in_fence());
}

#[test]
fn fence_tracker_ignores_shorter_closing_marker() {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe("````"));
    assert!(tracker.in_fence());
    assert!(tracker.observe("```"));
    assert!(tracker.in_fence());
}

#[test]
fn fence_tracker_requires_matching_marker_to_close() {
    let mut tracker = FenceTracker::default();
    assert!(tracker.observe("```"));
    assert!(tracker.in_fence());
    assert!(tracker.observe("~~~"));
    assert!(tracker.in_fence());
    assert!(tracker.observe("````"));
    assert!(!tracker.in_fence());
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
    let results: Vec<bool> = lines.iter().map(|line| tracker.observe(line)).collect();
    assert_eq!(
        results,
        vec![true, true, false, true, false, true, false],
        "expected fences to be recognised with inline markers and atypical spacing"
    );
    assert!(
        !tracker.in_fence(),
        "tracker should end outside of a fence after matching closures"
    );
}

#[test]
fn fence_tracker_handles_tilde_fences() {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe("~~~~rust"));
    assert!(tracker.in_fence());
    assert!(tracker.observe("~~~~"));
    assert!(!tracker.in_fence());
}

#[rstest]
#[case("`")]
#[case("``")]
#[case("`~~`")]
#[case("~~`")]
#[case("`` ~~")]
fn fence_tracker_rejects_short_or_mixed_markers(#[case] line: &str) {
    let mut tracker = FenceTracker::default();
    assert!(!tracker.observe(line));
    assert!(!tracker.in_fence());
}
