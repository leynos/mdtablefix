//! Shared property-test strategies for inline date token handling.
//!
//! Unit and integration tests use these generators to exercise the same date
//! token shapes that production span grouping recognizes. Keeping the
//! strategies beside the inline wrapper avoids a second month list or a second
//! day/year generator in the test tree while still compiling them only for
//! test targets.

use proptest::prelude::*;

use super::month_names::MONTH_NAMES;

/// Generates accepted `MONTH_NAMES` values with random ASCII character casing.
///
/// This exercises case-insensitive matching while keeping generated names tied
/// to the production month-name source. Example: may produce `"jAn"`.
pub(crate) fn month_name_strategy() -> BoxedStrategy<String> {
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
pub(crate) fn ordinal_suffix_strategy() -> BoxedStrategy<&'static str> {
    prop_oneof![Just("st"), Just("nd"), Just("rd"), Just("th")].boxed()
}

/// Generates ordinal day tokens in the accepted 1 through 31 range.
///
/// The suffix is varied independently of the number to match current predicate
/// behaviour. Example: may produce `"22st"`.
pub(crate) fn ordinal_day_strategy() -> BoxedStrategy<String> { ordinal_day_with_range(1u8..=31) }

/// Generates ordinal day tokens for the supplied day range.
///
/// This supports valid and invalid predicate properties without duplicating
/// suffix handling. Example: with `1u8..=1`, may produce `"1st"`.
pub(crate) fn ordinal_day_with_range(
    range: impl Strategy<Value = u8> + 'static,
) -> BoxedStrategy<String> {
    (range, ordinal_suffix_strategy())
        .prop_map(|(day, suffix)| format!("{day}{suffix}"))
        .boxed()
}

/// Generates numeric day tokens in the accepted 1 through 31 range.
///
/// The optional comma covers prose forms such as `4,`. Example: may produce
/// `"19"` or `"19,"`.
pub(crate) fn numeric_day_strategy() -> BoxedStrategy<String> { numeric_day_with_range(1u8..=31) }

/// Generates numeric day tokens for the supplied day range.
///
/// This supports valid and invalid predicate properties without duplicating
/// comma handling. Example: with `4u8..=4`, may produce `"4,"`.
pub(crate) fn numeric_day_with_range(
    range: impl Strategy<Value = u8> + 'static,
) -> BoxedStrategy<String> {
    (range, any::<bool>())
        .prop_map(|(day, append_comma)| {
            if append_comma {
                format!("{day},")
            } else {
                day.to_string()
            }
        })
        .boxed()
}

/// Generates year tokens in the accepted 1000 through 2999 range.
///
/// This covers every accepted four-digit year value. Example: may produce
/// `"2025"`.
pub(crate) fn year_strategy() -> BoxedStrategy<String> {
    (1000u16..=2999).prop_map(|year| year.to_string()).boxed()
}
