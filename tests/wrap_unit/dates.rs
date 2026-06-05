//! `wrap_text` tests covering date-like sequence atomicity.
//!
//! These cases ensure common English date strings such as
//! `25th December 2025` are treated as contiguous inline fragments by the
//! wrapping logic, so they are not split across lines when the configured width
//! allows the full date to fit. The matcher assumes dates appear as adjacent
//! day, month, and year token sequences separated by whitespace, including
//! ordinal suffixes, numeric days with commas, and full or abbreviated month
//! names. Very narrow widths are only a resilience case: the wrapper may emit
//! an over-width date fragment just as it does for other long atomic tokens.
//! This trades a little line-fill tightness for preserving prose dates.

use mdtablefix::wrap::wrap_text;
use rstest::rstest;

#[rstest]
#[case("25th December 2025")]
#[case("19 March 2018")]
#[case("July 4, 2008")]
#[case("25th Dec 2025")]
#[case("Jul 4, 2008")]
fn wrap_text_keeps_date_sequence_intact(#[case] expected_date: &str) {
    let input = lines_vec![format!(
        "This paragraph has enough preceding prose to make {expected_date} a tempting wrap point."
    )];

    // Width 48 forces `wrap_text` to choose near the date in this input while
    // still leaving every supported date format short enough to fit intact.
    let output = wrap_text(&input, 48);

    assert!(output.iter().any(|line| line.contains(expected_date)));
}

#[test]
fn wrap_text_handles_date_wider_than_width() {
    let input = lines_vec!["Remember 25th December 2025 when wrapping."];
    // Width 10 is intentionally narrower than `25th December 2025`, so this
    // exercises the long-atomic-token fallback path rather than normal fitting.
    let output = wrap_text(&input, 10);

    assert!(!output.is_empty());
    assert!(
        output
            .iter()
            .any(|line| line.contains("25th December 2025")),
        "over-width date should fall back to long-token emission: {output:?}"
    );
}

#[test]
fn wrap_text_date_at_exact_boundary() {
    let input = lines_vec!["Then 25th December 2025 follows."];
    // Width 23 equals the display width of `Then 25th December 2025`, so the
    // date should fit exactly without forcing an internal break.
    let output = wrap_text(&input, 23);

    assert!(output.iter().any(|line| line == "Then 25th December 2025"));
}

#[test]
fn wrap_text_partial_date_not_grouped() {
    let input = lines_vec!["Plan around December 2025 carefully."];
    // Width 21 forces a break inside `December 2025` unless that partial date
    // is incorrectly grouped as an atomic fragment.
    let output = wrap_text(&input, 21);

    assert!(!output.iter().any(|line| line.contains("December 2025")));
    // Rejoining with single spaces checks content preservation after wrapping;
    // these prose fixtures deliberately avoid significant repeated spacing.
    assert_eq!(output.join(" "), input[0]);
}
