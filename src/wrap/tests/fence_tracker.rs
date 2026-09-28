//! Tests for the `FenceTracker` helper.
//!
//! These cases exercise fence detection across various markers and spacing so
//! the wrapper skips reflow inside fenced code blocks.

use proptest::prelude::*;
use rstest::rstest;

use crate::wrap::{
    FenceTracker,
    KernelState,
    LineFeatures,
    Region,
    classify_regions,
    compression_changes_region,
    is_fence,
    opener,
};

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

#[test]
fn source_line_observation_reports_transition_and_resulting_state() {
    let mut tracker = FenceTracker::new();

    let opening = tracker.observe_source_line("> > ```rust");
    assert!(!opening.was_in_fence);
    assert!(opening.is_fence_marker);
    assert!(opening.is_in_fence);

    let content = tracker.observe_source_line("> > code");
    assert!(content.was_in_fence);
    assert!(!content.is_fence_marker);
    assert!(content.is_in_fence);

    let shallower = tracker.observe_source_line("> prose");
    assert!(!shallower.was_in_fence);
    assert!(!shallower.is_fence_marker);
    assert!(!shallower.is_in_fence);
}

#[test]
fn observe_source_fence_exposes_structural_marker_with_prefix_indent() {
    let mut tracker = FenceTracker::new();

    let opening = tracker.observe_source_fence("> > ```rust");
    assert!(opening.observation.is_fence_marker);
    assert!(opening.observation.is_in_fence);
    assert_eq!(opening.fence, Some(("> > ", "```", "rust")));
    assert_eq!(
        opening.features,
        Some(LineFeatures::fence(2, '`', 3, false))
    );

    let content = tracker.observe_source_fence("> > code");
    assert!(content.observation.is_in_fence);
    assert!(!content.observation.is_fence_marker);
    assert!(content.fence.is_none());
    assert_eq!(content.features, None);

    let closing = tracker.observe_source_fence("> > ```");
    assert!(closing.observation.is_fence_marker);
    assert!(!closing.observation.is_in_fence);
    assert_eq!(closing.fence, Some(("> > ", "```", "")));
    assert_eq!(closing.features, Some(LineFeatures::fence(2, '`', 3, true)));
}

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

#[test]
fn fence_tracker_treats_info_string_marker_as_content_not_close() {
    // Per CommonMark, a closing fence must not carry an info string. A
    // same-marker line bearing trailing text is literal content and leaves the
    // fence open, while a bare (or whitespace-only) marker still closes it.
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe("```rust", 0));
    assert!(tracker.in_fence(0));

    // Same marker, but a non-whitespace info string: not a close.
    assert!(tracker.observe("```rust", 0));
    assert!(tracker.in_fence(0));

    // A bare marker closes as usual.
    assert!(tracker.observe("```", 0));
    assert!(!tracker.in_fence(0));
}

#[rstest]
#[case("```\u{a0}")]
#[case("```\u{c}")]
fn fence_tracker_rejects_non_ascii_whitespace_close(#[case] closing: &str) {
    // CommonMark permits only ASCII spaces and tabs after a closing marker, so
    // a no-break space (U+00A0) or form feed (U+000C) is literal content and
    // must not close the fence.
    let mut tracker = FenceTracker::new();
    assert!(tracker.observe("```rust", 0));
    assert!(tracker.in_fence(0));

    assert!(tracker.observe(closing, 0));
    assert!(tracker.in_fence(0));

    // A bare marker still closes.
    assert!(tracker.observe("```", 0));
    assert!(!tracker.in_fence(0));
}

/// The two non-vacuity witnesses for the region-classification obligation.
///
/// Both documents open with four backticks and carry an interior three-backtick
/// line, which stays literal in both. The only difference is the final line, so
/// the pair pins down which property distinguishes the two outcomes: a trailing
/// four-backtick line closes the opener and is itself a delimiter, while a
/// trailing three-backtick line is too short to close it and so remains literal
/// interior content. The interior line's region is what the obligation is
/// about, and showing it unchanged across both closers is what stops the pair
/// from being a restatement of one case.
#[test]
fn regions_witness_wider_closer_stays_literal() {
    let lines = ["````", "```", "literal", "````"];
    assert_eq!(
        classify_regions(lines),
        vec![
            Region::Delim,
            Region::Literal,
            Region::Literal,
            Region::Delim
        ],
    );
}

#[test]
fn regions_witness_shorter_closer_leaves_the_opener_printed() {
    // The document ends inside the fence, so the interior three-backtick line
    // is literal content and the trailing line stays inside the same region.
    let lines = ["````", "```", "literal", "```"];
    assert_eq!(
        classify_regions(lines),
        vec![
            Region::Delim,
            Region::Literal,
            Region::Literal,
            Region::Literal
        ],
    );
}

/// The issue #480 reproduction, stated as a specification test.
///
/// A four-backtick opener followed by a three-backtick line and a payload line
/// must leave the payload literal. The defect this pins is a pass that reads
/// the interior three-backtick line as a closer and then rewrites the payload.
#[test]
fn regions_treats_payload_after_interior_shorter_fence_as_literal() {
    let lines = ["````", "```", "literal..."];
    assert_eq!(
        classify_regions(lines),
        vec![Region::Delim, Region::Literal, Region::Literal],
    );
}

/// The compression predicate flags exactly the interior lines a rewrite would
/// move out of the literal region.
///
/// Compressing a delimiter always writes three backticks, so an interior line
/// is a hazard precisely when its marker is a backtick, or the opener's own
/// family, and its run reaches the compressed opener's length. The two `true`
/// rows are that shape: an interior line the rewritten three-backtick opener
/// would newly close, ending the block early, which is issue #480. The two
/// `false` rows are the cases the pass must stay quiet about, because the
/// rewritten block's regions are unchanged either way.
#[rstest]
#[case::same_family_short_run(opener(0, '`', 4), LineFeatures::fence(0, '`', 3, true), true)]
#[case::tilde_opener_backtick_interior(
    opener(0, '~', 3),
    LineFeatures::fence(0, '`', 3, true),
    true
)]
#[case::unrelated_marker_family(opener(0, '`', 4), LineFeatures::fence(0, '~', 4, true), false)]
#[case::prose(opener(0, '`', 4), LineFeatures::prose(0), false)]
fn compression_changes_region_matches_the_rewrite_hazard(
    #[case] opening: KernelState,
    #[case] interior: LineFeatures,
    #[case] expected: bool,
) {
    assert_eq!(
        compression_changes_region(opening, interior),
        expected,
        "opening {opening:?} against interior {interior:?}",
    );
}
