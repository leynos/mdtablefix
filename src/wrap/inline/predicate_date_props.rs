//! Property tests for date predicate helpers.
//!
//! These tests verify that date component predicates recognise valid month
//! names, ordinal days, numeric days, and years while rejecting malformed or
//! out-of-range inputs. Property-based coverage exercises case variation,
//! boundary values, optional numeric-day commas, and arbitrary invalid strings
//! beyond the example-based predicate checks.

use proptest::prelude::*;

use super::{is_month_name, is_numeric_day, is_ordinal_day, is_year};

/// Accepted full and short month names used to check `is_month_name`.
///
/// `MONTH_NAMES` has 23 entries: 12 full names plus 11 short forms, because
/// `May` is identical in full and short form and is listed once.
const MONTH_NAMES: [&str; 23] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
];

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

/// Generates accepted month names with random ASCII character casing.
///
/// This exercises the case-insensitive `is_month_name` contract. Example: may
/// produce `"jAn"` or `"JAN"`.
fn month_name_strategy() -> BoxedStrategy<String> {
    prop::sample::select(&MONTH_NAMES)
        .prop_flat_map(|month| {
            prop::collection::vec(any::<bool>(), month.len()).prop_map(move |upper| {
                month
                    .chars()
                    .zip(upper)
                    .map(|(ch, is_upper)| {
                        if is_upper {
                            ch.to_ascii_uppercase()
                        } else {
                            ch.to_ascii_lowercase()
                        }
                    })
                    .collect()
            })
        })
        .boxed()
}

/// Generates the ordinal suffixes accepted by `is_ordinal_day`.
///
/// The suffix set is intentionally syntactic, not calendar-aware. Example:
/// may produce `"st"`.
fn ordinal_suffix_strategy() -> BoxedStrategy<&'static str> {
    prop_oneof![Just("st"), Just("nd"), Just("rd"), Just("th")].boxed()
}

/// Generates ordinal day tokens in the accepted 1 through 31 range.
///
/// The suffix is varied independently of the number to match current predicate
/// behaviour. Example: may produce `"22st"`.
fn ordinal_day_strategy() -> BoxedStrategy<String> {
    (1u8..=31, ordinal_suffix_strategy())
        .prop_map(|(day, suffix)| format!("{day}{suffix}"))
        .boxed()
}

/// Generates ordinal day tokens outside the accepted range.
///
/// Includes zero and every `u8` value above 31 to exercise boundary rejection.
/// Example: may produce `"32nd"`.
fn ordinal_day_out_of_range_strategy() -> BoxedStrategy<String> {
    (
        prop_oneof![Just(0u8), (32u8..=u8::MAX)],
        ordinal_suffix_strategy(),
    )
        .prop_map(|(day, suffix)| format!("{day}{suffix}"))
        .boxed()
}

/// Generates numeric day tokens in the accepted 1 through 31 range.
///
/// The optional comma covers prose forms such as `4,`. Example: may produce
/// `"19"` or `"19,"`.
fn numeric_day_strategy() -> BoxedStrategy<String> {
    (1u8..=31, any::<bool>())
        .prop_map(|(day, append_comma)| format_day_with_optional_comma(day, append_comma))
        .boxed()
}

/// Generates numeric day tokens outside the accepted range.
///
/// Includes zero and every `u8` value above 31, with and without a trailing
/// comma. Example: may produce `"0,"`.
fn numeric_day_out_of_range_strategy() -> BoxedStrategy<String> {
    (prop_oneof![Just(0u8), (32u8..=u8::MAX)], any::<bool>())
        .prop_map(|(day, append_comma)| format_day_with_optional_comma(day, append_comma))
        .boxed()
}

/// Formats a day number with the optional comma used by numeric-day strategies.
///
/// This keeps valid and invalid numeric-day generation consistent. Example:
/// `format_day_with_optional_comma(4, true)` returns `"4,"`.
fn format_day_with_optional_comma(day: u8, append_comma: bool) -> String {
    if append_comma {
        format!("{day},")
    } else {
        day.to_string()
    }
}

/// Generates year tokens in the accepted 1000 through 2999 range.
///
/// This covers every accepted four-digit year value. Example: may produce
/// `"2025"`.
fn year_strategy() -> BoxedStrategy<String> {
    (1000u16..=2999).prop_map(|year| year.to_string()).boxed()
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
        prop_assume!(!MONTH_NAMES.iter().any(|month| token.eq_ignore_ascii_case(month)));
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
