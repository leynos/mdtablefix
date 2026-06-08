//! Snapshot tests for date-like sequence atomicity during paragraph wrapping.

use mdtablefix::wrap::wrap_text;
use rstest::rstest;

fn wrap_lines(input: &str, width: usize) -> String {
    let lines: Vec<String> = input.lines().map(str::to_owned).collect();
    wrap_text(&lines, width).join("\n")
}

fn assert_wrap_snapshot(name: &str, input: &str, width: usize) {
    insta::with_settings!({
        snapshot_path => "../snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!(name, wrap_lines(input, width));
    });
}

#[rstest]
#[case(
    "date_wrap_ordinal_day_month_year",
    "Remember 25th December 2025 when wrapping prose near a boundary.",
    28
)]
#[case(
    "date_wrap_numeric_day_month_year",
    "The archive references 19 March 2018 before the next paragraph begins.",
    24
)]
#[case(
    "date_wrap_month_day_year_with_comma",
    "The record cites July 4, 2008, before the follow-up note.",
    24
)]
#[case(
    "date_wrap_parenthesised_with_footnote",
    "See (July 4, 2008).[^1] before editing the release note.",
    24
)]
#[case("date_wrap_over_width_fallback", "25th December 2025.", 10)]
fn date_sequence_wrap_snapshots(#[case] name: &str, #[case] input: &str, #[case] width: usize) {
    assert_wrap_snapshot(name, input, width);
}
