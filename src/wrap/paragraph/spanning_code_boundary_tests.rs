//! Regression tests for invalid byte ranges in spanning code helpers.

use super::{
    OverlongSpan,
    overlong_span_crossing_boundaries,
    preserve_span_boundaries,
    split_span_at_boundaries,
};

#[rstest::rstest]
#[case::mid_scalar(2, 6)]
#[case::past_end(0, 7)]
fn overlong_span_rejects_invalid_text_ranges(#[case] start: usize, #[case] end: usize) {
    let text = "`é x`";
    let boundaries = [4];

    assert!(overlong_span_crossing_boundaries(text, &boundaries, start..end, 0).is_none());
}

#[rstest::rstest]
#[case::mid_scalar(2)]
#[case::past_end(6)]
fn split_span_rejects_invalid_authored_boundaries(#[case] boundary: usize) {
    let text = "`éx`";

    assert!(split_span_at_boundaries(text, 0, text.len(), &[boundary]).is_none());
}

#[rstest::rstest]
#[case::mid_scalar(2, 4)]
#[case::past_end(0, 5)]
fn preserve_span_boundaries_keeps_original_lines_for_invalid_ranges(
    #[case] start: usize,
    #[case] end: usize,
) {
    let joined = "`é`";
    let mut lines = vec!["before `é` after".to_owned()];
    let original_lines = lines.clone();

    preserve_span_boundaries(
        &mut lines,
        joined,
        OverlongSpan {
            range: start..end,
            pieces: vec!["replacement".to_owned()],
        },
        20,
    );

    assert_eq!(lines, original_lines);
}
