//! Shared property-test strategies for date-like wrap fixtures.

use mdtablefix::MONTH_NAMES;
use proptest::prelude::*;

/// Generates full and abbreviated English month names.
///
/// Examples include `"January"`, `"Feb"`, and `"May"`.
pub(crate) fn month_name_strategy() -> BoxedStrategy<String> {
    prop::sample::select(&MONTH_NAMES)
        .prop_map(str::to_string)
        .boxed()
}

/// Generates ordinal day strings from 1 through 31 for wrap testing.
///
/// This exercises prose token grouping and does not validate calendar-specific
/// month/day combinations.
fn ordinal_day_strategy() -> BoxedStrategy<String> {
    (1u8..=31)
        .prop_map(|day| format!("{day}{}", ordinal_suffix(day)))
        .boxed()
}

/// Returns the English ordinal suffix for `day`.
///
/// The 11 through 13 values use `th` (`11th`, `12th`, `13th`) instead of the
/// last-digit rule.
fn ordinal_suffix(day: u8) -> &'static str {
    match day {
        11..=13 => "th",
        _ if day % 10 == 1 => "st",
        _ if day % 10 == 2 => "nd",
        _ if day % 10 == 3 => "rd",
        _ => "th",
    }
}

/// Generates numeric day strings from 1 through 31 for wrap testing.
///
/// The optional trailing comma exercises punctuation-adjacent dates. This is a
/// token-shape strategy and intentionally does not validate calendar-specific
/// month/day combinations.
fn numeric_day_strategy() -> BoxedStrategy<String> {
    (1u8..=31, any::<bool>())
        .prop_map(|(day, append_comma)| {
            if append_comma {
                format!("{day},")
            } else {
                day.to_string()
            }
        })
        .boxed()
}

/// Generates year strings in the accepted 1000 through 2999 range.
///
/// Examples include `"1066"`, `"2024"`, and `"2999"`.
pub(crate) fn year_strategy() -> BoxedStrategy<String> {
    (1000u16..=2999).prop_map(|year| year.to_string()).boxed()
}

/// Generates tokenized date sequences separated by literal space tokens.
///
/// The strategy covers ordinal day first, numeric day first, and month first
/// variants. Examples include `["1st", " ", "January", " ", "2024"]`,
/// `["1", " ", "January", " ", "2024"]`, and
/// `["January", " ", "1", " ", "2024"]`. Numeric day variants can emit a
/// trailing comma, such as `"1,"`, to exercise punctuation-adjacent dates.
pub(crate) fn date_sequence_tokens_strategy() -> BoxedStrategy<Vec<String>> {
    prop_oneof![
        (
            ordinal_day_strategy(),
            month_name_strategy(),
            year_strategy()
        )
            .prop_map(|(day, month, year)| vec![day, " ".into(), month, " ".into(), year]),
        (
            numeric_day_strategy(),
            month_name_strategy(),
            year_strategy()
        )
            .prop_map(|(day, month, year)| vec![day, " ".into(), month, " ".into(), year]),
        (
            month_name_strategy(),
            numeric_day_strategy(),
            year_strategy()
        )
            .prop_map(|(month, day, year)| vec![month, " ".into(), day, " ".into(), year]),
    ]
    .boxed()
}
