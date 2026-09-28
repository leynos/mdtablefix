//! Property coverage for the `FenceTracker` transition rules.
//!
//! These cases generalise the behavioural examples in `fence_tracker` over
//! marker family, run length, blockquote depth, and info strings, so the rules
//! hold for the whole shape space rather than the sampled rows. They live apart
//! from the behavioural cases to keep each test module within the repository's
//! file-size limit.

use proptest::prelude::*;

use crate::wrap::FenceTracker;

/// Build a blockquote-prefixed source line at the requested nesting depth.
fn quoted_line(depth: usize, body: &str) -> String { format!("{}{body}", "> ".repeat(depth)) }

/// Return the marker character that is *not* `marker`, so property tests can
/// construct a fence run with an incompatible delimiter.
fn other_marker(marker: char) -> char { if marker == '`' { '~' } else { '`' } }

proptest! {
    /// A fence closes only when a compatible marker is observed at the exact
    /// depth it opened: the same delimiter character at a deeper nesting level
    /// is literal content and leaves the block open, while the same marker (of
    /// at least the opening length) at the opening depth closes it.
    #[test]
    fn observe_source_line_closes_only_with_compatible_marker_at_open_depth(
        open_depth in 0_usize..=4,
        marker in prop_oneof![Just('`'), Just('~')],
        open_len in 3_usize..=6,
        close_extra in 0_usize..=3,
        nested_offset in 1_usize..=2,
    ) {
        let run = marker.to_string();
        let mut tracker = FenceTracker::new();

        let opening = quoted_line(open_depth, &format!("{}rust", run.repeat(open_len)));
        let open_obs = tracker.observe_source_line(&opening);
        prop_assert!(open_obs.is_fence_marker);
        prop_assert!(!open_obs.was_in_fence);
        prop_assert!(open_obs.is_in_fence);

        // A compatible marker nested deeper than the opener matches fence-marker
        // syntax, but the tracker does not transition: the close is gated on the
        // opening depth, so a deeper line cannot close the fence. `is_in_fence`
        // therefore stays true, so the deeper line remains fenced content.
        let nested = quoted_line(open_depth + nested_offset, &run.repeat(open_len + close_extra));
        let nested_obs = tracker.observe_source_line(&nested);
        prop_assert!(nested_obs.is_fence_marker);
        prop_assert!(nested_obs.was_in_fence);
        prop_assert!(nested_obs.is_in_fence);

        // The same marker (length >= opening length) at the opening depth closes.
        let closing = quoted_line(open_depth, &run.repeat(open_len + close_extra));
        let close_obs = tracker.observe_source_line(&closing);
        prop_assert!(close_obs.is_fence_marker);
        prop_assert!(close_obs.was_in_fence);
        prop_assert!(!close_obs.is_in_fence);
    }

    /// An incompatible marker at the opening depth — a different delimiter
    /// character or a shorter run — is recognised as a fence line but does not
    /// close the active block.
    #[test]
    fn observe_source_line_ignores_incompatible_marker_at_open_depth(
        open_depth in 0_usize..=4,
        marker in prop_oneof![Just('`'), Just('~')],
        open_len in 4_usize..=6,
        wrong_char in any::<bool>(),
    ) {
        let mut tracker = FenceTracker::new();
        let opening = quoted_line(open_depth, &format!("{}rust", marker.to_string().repeat(open_len)));
        prop_assert!(tracker.observe_source_line(&opening).is_in_fence);

        let (close_char, close_len) = if wrong_char {
            (other_marker(marker), open_len)
        } else {
            (marker, open_len - 1)
        };
        let closing = quoted_line(open_depth, &close_char.to_string().repeat(close_len));
        let obs = tracker.observe_source_line(&closing);
        prop_assert!(obs.is_fence_marker);
        prop_assert!(obs.was_in_fence);
        prop_assert!(obs.is_in_fence);
    }

    /// Dropping to any depth shallower than `open_depth` implicitly closes the
    /// fence, regardless of the line's content.
    #[test]
    fn observe_source_line_closes_implicitly_when_depth_drops_below_open(
        open_depth in 1_usize..=4,
        marker in prop_oneof![Just('`'), Just('~')],
        open_len in 3_usize..=6,
        drop_offset in 1_usize..=4,
        inner in "[a-zA-Z0-9 _-]{0,20}",
    ) {
        let mut tracker = FenceTracker::new();
        let opening = quoted_line(open_depth, &format!("{}rust", marker.to_string().repeat(open_len)));
        prop_assert!(tracker.observe_source_line(&opening).is_in_fence);

        let shallower_depth = open_depth.saturating_sub(drop_offset);
        let shallower = quoted_line(shallower_depth, &inner);
        let obs = tracker.observe_source_line(&shallower);
        prop_assert!(!obs.was_in_fence);
        prop_assert!(!obs.is_fence_marker);
        prop_assert!(!obs.is_in_fence);
    }

    /// A same-marker line carrying a non-blank info string is literal content,
    /// not a closing fence, so the fence stays open regardless of marker length
    /// or nesting depth. A subsequent bare marker at the opening depth closes it.
    #[test]
    fn observe_source_line_treats_info_string_marker_as_content(
        open_depth in 0_usize..=4,
        marker in prop_oneof![Just('`'), Just('~')],
        open_len in 3_usize..=6,
        extra in 0_usize..=3,
        info in "[a-zA-Z][a-zA-Z0-9 ]{0,10}",
    ) {
        let run = marker.to_string();
        let mut tracker = FenceTracker::new();

        let opening = quoted_line(open_depth, &format!("{}rust", run.repeat(open_len)));
        prop_assert!(tracker.observe_source_line(&opening).is_in_fence);

        // Same marker and depth, at least the opening length, but with an info
        // string: not a valid close, so the fence remains open.
        let info_marker = quoted_line(open_depth, &format!("{}{info}", run.repeat(open_len + extra)));
        let obs = tracker.observe_source_line(&info_marker);
        prop_assert!(obs.was_in_fence);
        prop_assert!(obs.is_in_fence);

        // A bare marker at the opening depth closes as usual.
        let closing = quoted_line(open_depth, &run.repeat(open_len));
        prop_assert!(!tracker.observe_source_line(&closing).is_in_fence);
    }
}
