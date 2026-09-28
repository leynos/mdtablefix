//! Tests for the `FenceTracker` helper.
//!
//! These cases exercise fence detection across various markers and spacing so
//! the wrapper skips reflow inside fenced code blocks. Kernel-level cases — the
//! batch `classify_regions` fold and the compression predicate — live in
//! `fence_kernel_tests`, the transition-logging cases in
//! `fence_tracker_logging`, and the property cases in `fence_tracker_props`.

use rstest::rstest;

use crate::wrap::{FenceObservation, FenceTracker, LineFeatures, ObservedFence, is_fence};

/// Asserts the tracker's fence state at `depth`, without stepping a line.
fn assert_fence_state(tracker: &FenceTracker, depth: usize, expected: bool) {
    assert_eq!(
        tracker.in_fence(depth),
        expected,
        "the fence state at depth {depth}",
    );
}

/// Steps the tracker over `line` and asserts the state it leaves behind.
///
/// Nearly every case in this module is a short sequence of lines whose only
/// observable effect is the tracker's fence state, so naming that pairing keeps
/// the cases reading as the rules they state. `in_fence` is the state expected
/// afterwards: a closer leaves `false`, while an opener — and any marker too
/// weak to close the active block — leaves `true`.
fn assert_fence_step(tracker: &mut FenceTracker, line: &str, depth: usize, in_fence: bool) {
    assert!(
        tracker.observe(line, depth),
        "{line:?} should be recognised as a fence marker",
    );
    assert_fence_state(tracker, depth, in_fence);
}

/// Asserts that one source line yields exactly the expected transition.
fn assert_transition(
    step: &FenceObservation,
    was_in_fence: bool,
    is_fence_marker: bool,
    is_in_fence: bool,
) {
    assert_eq!(step.was_in_fence, was_in_fence, "the entry fence state");
    assert_eq!(
        step.is_fence_marker, is_fence_marker,
        "the fence-marker flag"
    );
    assert_eq!(step.is_in_fence, is_in_fence, "the exit fence state");
}

/// Asserts a line's marker components and kernel view in one step.
///
/// `observe_source_fence` reports three parallel things about a line: the
/// transition, the parsed `(indent, marker, info)` components, and the kernel's
/// own view of the line. Asserting them together keeps callers to one line per
/// observed step and makes the expected shape of each field explicit.
fn assert_fenced_line(
    observed: &ObservedFence<'_>,
    expected_fence: Option<(&str, &str, &str)>,
    expected_features: Option<LineFeatures>,
) {
    assert_eq!(
        observed.fence, expected_fence,
        "the parsed marker components"
    );
    assert_eq!(observed.features, expected_features, "the kernel line view");
}

#[test]
fn fence_tracker_new_starts_outside_fence() {
    let tracker = FenceTracker::new();
    assert!(!tracker.in_fence(0));
}

#[test]
fn fence_tracker_closes_matching_markers() {
    let mut tracker = FenceTracker::default();
    assert_fence_state(&tracker, 0, false);
    assert_fence_step(&mut tracker, "```rust", 0, true);
    assert_fence_step(&mut tracker, "```", 0, false);
}

#[test]
fn fence_tracker_closes_with_info_string() {
    let mut tracker = FenceTracker::new();
    assert_fence_step(&mut tracker, "```rust", 0, true);
    // Trailing spaces are permitted after a closing marker.
    assert_fence_step(&mut tracker, "```   ", 0, false);
}

#[test]
fn fence_tracker_ignores_shorter_closing_marker() {
    let mut tracker = FenceTracker::new();
    assert_fence_step(&mut tracker, "````", 0, true);
    // A three-backtick run cannot close a four-backtick opener.
    assert_fence_step(&mut tracker, "```", 0, true);
}

#[test]
fn fence_tracker_requires_matching_marker_to_close() {
    let mut tracker = FenceTracker::default();
    assert_fence_step(&mut tracker, "```", 0, true);
    // A tilde run is a marker of a different family, so it cannot close.
    assert_fence_step(&mut tracker, "~~~", 0, true);
    assert_fence_step(&mut tracker, "````", 0, false);
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
    assert_fence_state(&tracker, 0, false);
}

#[test]
fn fence_tracker_handles_tilde_fences() {
    let mut tracker = FenceTracker::new();
    assert_fence_step(&mut tracker, "~~~~rust", 0, true);
    assert_fence_step(&mut tracker, "~~~~", 0, false);
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
    assert_fence_step(&mut tracker, outer_start, 0, true);
    // An inner run of a different family, or a shorter same-family run, is
    // literal content: it is recognised as a marker but does not close.
    assert_fence_step(&mut tracker, inner_start, 0, true);
    assert_fence_step(&mut tracker, inner_end, 0, true);
    assert!(tracker.observe(outer_end, 0));
    assert_fence_state(&tracker, 0, expected_final_in_fence);
}

#[rstest]
#[case("`")]
#[case("``")]
#[case("`~~`")]
#[case("~~`")]
#[case("`` ~~")]
fn fence_tracker_rejects_short_or_mixed_markers(#[case] line: &str) {
    let mut tracker = FenceTracker::default();
    assert!(!tracker.observe(line, 0), "{line:?} is not a fence marker");
    assert_fence_state(&tracker, 0, false);
}

#[test]
fn fence_tracker_opens_and_closes_at_nested_depth() {
    let mut tracker = FenceTracker::new();
    assert_fence_step(&mut tracker, "```rust", 2, true);
    assert_fence_step(&mut tracker, "```", 2, false);
}

#[test]
fn fence_tracker_closes_when_blockquote_depth_decreases() {
    let mut tracker = FenceTracker::new();
    assert_fence_step(&mut tracker, "```rust", 2, true);
    // Dropping to a shallower depth closes the block implicitly.
    assert!(!tracker.observe("plain text", 1));
    assert_fence_state(&tracker, 1, false);
}

#[test]
fn fence_tracker_remains_open_for_deeper_content() {
    let mut tracker = FenceTracker::new();
    assert_fence_step(&mut tracker, "```rust", 1, true);
    // Content at a deeper depth is inside the block, not a delimiter.
    assert!(!tracker.observe("plain text", 2));
    assert_fence_state(&tracker, 2, true);
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
    assert!(tracker.observe_line(opening), "{opening:?} opens a block");
    assert!(
        tracker.in_fence_for_line(opening),
        "the opening line is inside"
    );
    assert_fence_state(&tracker, depth, true);
}

#[test]
fn raw_blockquote_fence_closes_when_quote_depth_decreases() {
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe_line("> > ```rust"));
    assert!(!tracker.observe_line("> ordinary quote text"));
    assert!(!tracker.in_fence_for_line("> ordinary quote text"));
}

#[test]
fn source_line_observation_reports_transition_and_resulting_state() {
    let mut tracker = FenceTracker::new();

    let opening = tracker.observe_source_line("> > ```rust");
    assert_transition(&opening, false, true, true);

    let content = tracker.observe_source_line("> > code");
    assert_transition(&content, true, false, true);

    // Dropping below the opening depth closes the block implicitly, so the line
    // is neither a marker nor inside a fence.
    let shallower = tracker.observe_source_line("> prose");
    assert_transition(&shallower, false, false, false);
}

#[test]
fn observe_source_fence_exposes_structural_marker_with_prefix_indent() {
    let mut tracker = FenceTracker::new();

    let opening = tracker.observe_source_fence("> > ```rust");
    assert!(opening.observation.is_fence_marker);
    assert!(opening.observation.is_in_fence);
    assert_fenced_line(
        &opening,
        Some(("> > ", "```", "rust")),
        Some(LineFeatures::fence(2, '`', 3, false)),
    );

    // A non-marker line inside the block carries no marker components and no
    // kernel view, but it is still inside the fence.
    let content = tracker.observe_source_fence("> > code");
    assert!(!content.observation.is_fence_marker);
    assert!(content.observation.is_in_fence);
    assert_fenced_line(&content, None, None);

    // The closing line reuses the opener's prefix and marker but carries no
    // info string, and its trailing-blank flag is set because nothing follows
    // the run.
    let closing = tracker.observe_source_fence("> > ```");
    assert!(closing.observation.is_fence_marker);
    assert!(!closing.observation.is_in_fence);
    assert_fenced_line(
        &closing,
        Some(("> > ", "```", "")),
        Some(LineFeatures::fence(2, '`', 3, true)),
    );
}

#[test]
fn fence_tracker_treats_info_string_marker_as_content_not_close() {
    // Per CommonMark, a closing fence must not carry an info string. A
    // same-marker line bearing trailing text is literal content and leaves the
    // fence open, while a bare (or whitespace-only) marker still closes it.
    let mut tracker = FenceTracker::new();
    assert_fence_step(&mut tracker, "```rust", 0, true);

    // Same marker, but a non-whitespace info string: not a close.
    assert_fence_step(&mut tracker, "```rust", 0, true);

    // A bare marker closes as usual.
    assert_fence_step(&mut tracker, "```", 0, false);
}

#[rstest]
#[case("```\u{a0}")]
#[case("```\u{c}")]
fn fence_tracker_rejects_non_ascii_whitespace_close(#[case] closing: &str) {
    // CommonMark permits only ASCII spaces and tabs after a closing marker, so
    // a no-break space (U+00A0) or form feed (U+000C) is literal content and
    // must not close the fence.
    let mut tracker = FenceTracker::new();
    assert_fence_step(&mut tracker, "```rust", 0, true);

    assert_fence_step(&mut tracker, closing, 0, true);

    // A bare marker still closes.
    assert_fence_step(&mut tracker, "```", 0, false);
}
