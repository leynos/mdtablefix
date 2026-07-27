//! Transition-logging tests for the `FenceTracker` helper.
//!
//! These cases assert that fence-state transitions emit stable, content-free
//! tracing events, so operators can diagnose fence handling without the logs
//! leaking document text. They live apart from the behavioural cases in
//! `fence_tracker` to keep each test module within the repository's file-size
//! limit.

use tracing_test::traced_test;

use crate::wrap::FenceTracker;

#[traced_test]
#[test]
fn fence_opening_logs_content_free_transition() {
    let input = "```private-opening-info";
    let mut tracker = FenceTracker::new();

    assert!(tracker.observe(input, 2));
    assert!(logs_contain("transition=\"open\""));
    assert!(logs_contain("depth=2"));
    assert!(logs_contain("open_depth=2"));
    assert!(logs_contain("marker_len=3"));
    assert!(!logs_contain(input));
    assert!(!logs_contain("private-opening-info"));
}

#[traced_test]
#[test]
fn matching_fence_closure_logs_content_free_transition() {
    let opening = "````private-opening-info";
    let closing = "````";
    let mut tracker = FenceTracker::new();

    assert!(tracker.observe(opening, 1));
    assert!(tracker.observe(closing, 1));
    assert!(logs_contain("transition=\"matching_close\""));
    assert!(logs_contain("depth=1"));
    assert!(logs_contain("open_depth=1"));
    assert!(logs_contain("marker_len=4"));
    assert!(logs_contain("open_marker_len=4"));
    assert!(!logs_contain(opening));
    assert!(!logs_contain("private-opening-info"));
}

#[traced_test]
#[test]
fn info_string_marker_logs_content_free_unchanged_transition() {
    let opening = "````private-opening-info";
    let info_marker = "````private-closing-info";
    let mut tracker = FenceTracker::new();

    assert!(tracker.observe(opening, 1));
    // A same-length, same-marker line with an info string does not close.
    assert!(tracker.observe(info_marker, 1));
    assert!(tracker.in_fence(1));
    assert!(logs_contain("transition=\"unchanged\""));
    assert!(logs_contain("reason=\"closing_fence_has_info_string\""));
    assert!(!logs_contain(opening));
    assert!(!logs_contain(info_marker));
    assert!(!logs_contain("private-closing-info"));
}

#[traced_test]
#[test]
fn depth_decrease_logs_content_free_implicit_closure() {
    let opening = "```private-opening-info";
    let shallower_line = "private shallower payload";
    let mut tracker = FenceTracker::new();

    assert!(tracker.observe(opening, 3));
    assert!(!tracker.observe(shallower_line, 2));
    assert!(logs_contain("transition=\"implicit_close\""));
    assert!(logs_contain("reason=\"blockquote_depth_decreased\""));
    assert!(logs_contain("depth=2"));
    assert!(logs_contain("open_depth=3"));
    assert!(logs_contain("open_marker_len=3"));
    assert!(!logs_contain(opening));
    assert!(!logs_contain(shallower_line));
}

#[traced_test]
#[test]
fn incompatible_marker_logs_content_free_unchanged_transition() {
    let opening = "````private-opening-info";
    let incompatible = "~~~private-incompatible-info";
    let mut tracker = FenceTracker::new();

    assert!(tracker.observe(opening, 1));
    assert!(tracker.observe(incompatible, 1));
    assert!(logs_contain("transition=\"unchanged\""));
    assert!(logs_contain("reason=\"incompatible_active_opener\""));
    assert!(logs_contain("marker_len=3"));
    assert!(logs_contain("open_marker_len=4"));
    assert!(!logs_contain(opening));
    assert!(!logs_contain(incompatible));
}
