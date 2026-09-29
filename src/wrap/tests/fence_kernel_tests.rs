//! Kernel-level specification tests for fence classification.
//!
//! These cases exercise the pure transition kernel directly, rather than
//! through the streaming [`FenceTracker`](crate::wrap::FenceTracker): the
//! batch `classify_regions` fold and the `compression_changes_region`
//! predicate the compression pass consults. They live apart from the
//! behavioural cases in `fence_tracker` to keep each test module within the
//! repository's file-size limit.

use rstest::rstest;

use crate::wrap::{
    KernelState,
    LineFeatures,
    Region,
    classify_regions,
    compression_changes_region,
    opener,
};

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
