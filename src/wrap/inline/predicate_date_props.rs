//! Property tests for date predicate helpers.
//!
//! These tests verify that date component predicates recognise valid month
//! names, ordinal days, numeric days, and years while rejecting malformed or
//! out-of-range inputs. Property-based coverage exercises case variation,
//! boundary values, optional numeric-day commas, and arbitrary invalid strings
//! beyond the example-based predicate checks.

use proptest::prelude::*;
use rstest::rstest;

use super::{
    super::date_strategies::{
        month_name_strategy,
        numeric_day_strategy,
        numeric_day_with_range,
        ordinal_day_strategy,
        ordinal_day_with_range,
        year_strategy,
    },
    MONTH_NAMES,
    is_month_name,
    is_numeric_day,
    is_opening_punct,
    is_ordinal_day,
    is_year,
};

/// Generates arbitrary 0 to 24 character strings for negative predicate tests.
///
/// The upper bound comfortably exceeds the longest month name while keeping
/// generated cases small enough for fast shrinking. Example: may produce
/// `"not-a-month"`.
fn arbitrary_short_string_strategy() -> BoxedStrategy<String> {
    prop::collection::vec(any::<char>(), 0..24)
        .prop_map(|chars| chars.into_iter().collect::<String>())
        .boxed()
}

fn strip_leading_openers(token: &str) -> &str { token.trim_start_matches(is_opening_punct) }

/// Generates ordinal day tokens outside the accepted range.
///
/// Includes zero and every `u8` value above 31 to exercise boundary rejection.
/// Example: may produce `"32nd"`.
fn ordinal_day_out_of_range_strategy() -> BoxedStrategy<String> {
    ordinal_day_with_range(prop_oneof![Just(0u8), (32u8..=u8::MAX)])
}

/// Generates numeric day tokens outside the accepted range.
///
/// Includes zero and every `u8` value above 31, with and without a trailing
/// comma. Example: may produce `"0,"`.
fn numeric_day_out_of_range_strategy() -> BoxedStrategy<String> {
    numeric_day_with_range(prop_oneof![Just(0u8), (32u8..=u8::MAX)])
}

/// Generates year tokens outside the accepted range.
///
/// Includes values below 1000 and above 2999 to exercise range rejection.
/// Example: may produce `"999"` or `"3000"`.
fn year_out_of_range_strategy() -> BoxedStrategy<String> {
    prop_oneof![(0u16..=999u16), (3000u16..=u16::MAX)]
        .prop_map(|year| year.to_string())
        .boxed()
}

#[rstest]
#[case("(July", true)]
#[case("\"December", true)]
#[case("(foo", false)]
fn is_month_name_strips_leading_openers(#[case] token: &str, #[case] expected: bool) {
    assert_eq!(is_month_name(token), expected);
}

#[rstest]
#[case("\"25th", true)]
#[case("(1st", true)]
#[case("(0th", false)]
fn is_ordinal_day_strips_leading_openers(#[case] token: &str, #[case] expected: bool) {
    assert_eq!(is_ordinal_day(token), expected);
}

#[rstest]
#[case("(4,", true)]
#[case("(19", true)]
#[case("(foo", false)]
fn is_numeric_day_strips_leading_openers(#[case] token: &str, #[case] expected: bool) {
    assert_eq!(is_numeric_day(token), expected);
}

proptest! {
    #[test]
    fn prop_is_month_name_accepts_canonical_names_case_insensitively(
        token in month_name_strategy(),
    ) {
        prop_assert!(is_month_name(&token));
    }

    #[test]
    fn prop_is_month_name_rejects_arbitrary_strings(
        token in arbitrary_short_string_strategy(),
    ) {
        let normalized = strip_leading_openers(&token);
        prop_assume!(!MONTH_NAMES
            .iter()
            .any(|month| normalized.eq_ignore_ascii_case(month)));
        prop_assert!(!is_month_name(&token));
    }

    #[test]
    fn prop_is_ordinal_day_accepts_valid_range(token in ordinal_day_strategy()) {
        prop_assert!(is_ordinal_day(&token));
    }

    #[test]
    fn prop_is_ordinal_day_rejects_out_of_range(
        token in ordinal_day_out_of_range_strategy(),
    ) {
        prop_assert!(!is_ordinal_day(&token));
    }

    #[test]
    fn prop_is_numeric_day_accepts_valid_range(token in numeric_day_strategy()) {
        prop_assert!(is_numeric_day(&token));
    }

    #[test]
    fn prop_is_numeric_day_rejects_out_of_range(
        token in numeric_day_out_of_range_strategy(),
    ) {
        prop_assert!(!is_numeric_day(&token));
    }

    #[test]
    fn prop_is_year_accepts_four_digit_range(token in year_strategy()) {
        prop_assert!(is_year(&token));
    }

    #[test]
    fn prop_is_year_rejects_out_of_range(token in year_out_of_range_strategy()) {
        prop_assert!(!is_year(&token));
    }
}
