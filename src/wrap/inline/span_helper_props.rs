//! Test-only property-test companion to the `span_helpers` module.
//!
//! It exercises `span_helpers::try_match_date_sequence` with the shared date
//! component generators from `date_strategies`, pairing each generated valid
//! date with equal-length near-misses. This companion exists to verify the
//! internal matcher contract directly: token meaning, order, separators, and
//! opening punctuation must determine a match rather than five-token length
//! alone.

use proptest::prelude::*;

use super::try_match_date_sequence;
use crate::wrap::inline::date_strategies::{
    month_name_strategy,
    numeric_day_strategy,
    ordinal_day_strategy,
    year_strategy,
};

fn whitespace_separator_strategy() -> BoxedStrategy<String> {
    prop::collection::vec(
        prop::sample::select(vec![
            ' ', '\t', '\n', '\r', '\u{00a0}', '\u{1680}', '\u{2003}', '\u{202f}', '\u{205f}',
            '\u{3000}',
        ]),
        1..=3,
    )
    .prop_map(|characters| characters.into_iter().collect())
    .boxed()
}

fn ordinal_day_month_year_strategy() -> BoxedStrategy<Vec<String>> {
    (
        ordinal_day_strategy(),
        whitespace_separator_strategy(),
        month_name_strategy(),
        whitespace_separator_strategy(),
        year_strategy(),
    )
        .prop_map(|(day, separator1, month, separator2, year)| {
            vec![day, separator1, month, separator2, year]
        })
        .boxed()
}

fn numeric_day_month_year_strategy() -> BoxedStrategy<Vec<String>> {
    (
        numeric_day_strategy(),
        whitespace_separator_strategy(),
        month_name_strategy(),
        whitespace_separator_strategy(),
        year_strategy(),
    )
        .prop_map(|(day, separator1, month, separator2, year)| {
            vec![day, separator1, month, separator2, year]
        })
        .boxed()
}

fn month_numeric_day_year_strategy() -> BoxedStrategy<Vec<String>> {
    (
        month_name_strategy(),
        whitespace_separator_strategy(),
        numeric_day_strategy(),
        whitespace_separator_strategy(),
        year_strategy(),
    )
        .prop_map(|(month, separator1, day, separator2, year)| {
            vec![month, separator1, day, separator2, year]
        })
        .boxed()
}

fn date_sequence_tokens_strategy() -> BoxedStrategy<Vec<String>> {
    prop_oneof![
        ordinal_day_month_year_strategy(),
        numeric_day_month_year_strategy(),
        month_numeric_day_year_strategy(),
    ]
    .boxed()
}

fn all_date_layouts_strategy() -> BoxedStrategy<Vec<Vec<String>>> {
    (
        ordinal_day_month_year_strategy(),
        numeric_day_month_year_strategy(),
        month_numeric_day_year_strategy(),
    )
        .prop_map(|(ordinal, numeric, month_first)| vec![ordinal, numeric, month_first])
        .boxed()
}

fn opening_prefix_strategy() -> BoxedStrategy<String> {
    prop::collection::vec(
        prop::sample::select(vec![
            '(', '[', '"', '“', '‘', '（', '［', '【', '《', '「', '『',
        ]),
        1..=3,
    )
    .prop_map(|characters| characters.into_iter().collect())
    .boxed()
}

fn invalid_opener_strategy() -> BoxedStrategy<char> {
    prop::sample::select(vec![
        ')', ']', '\'', '”', '’', '）', '］', '】', '》', '」', '』',
    ])
    .boxed()
}

fn preceding_tokens_strategy() -> BoxedStrategy<Vec<String>> {
    prop::collection::vec(
        prop::string::string_regex("[a-z]{1,8}")
            .expect("failed to build preceding token regex strategy"),
        0..=8,
    )
    .boxed()
}

#[test]
fn try_match_date_sequence_rejects_empty_slice() {
    assert_eq!(try_match_date_sequence(&[], 0), None);
}

proptest! {
    #[test]
    fn try_match_date_sequence_distinguishes_valid_dates_from_five_token_near_misses(
        (
            date_layouts,
            opening_prefix,
            invalid_opener,
            mut tokens,
        ) in (
            all_date_layouts_strategy(),
            opening_prefix_strategy(),
            invalid_opener_strategy(),
            preceding_tokens_strategy(),
        ),
    ) {
        let start = tokens.len();
        for (layout_index, mut date_tokens) in date_layouts.into_iter().enumerate() {
            date_tokens[0].insert_str(0, &opening_prefix);
            tokens.truncate(start);
            tokens.extend(date_tokens);

            prop_assert_eq!(
                try_match_date_sequence(&tokens, start),
                Some(start + 5),
                "valid date layout {} must match",
                layout_index,
            );

            let mut non_date_component = tokens.clone();
            non_date_component[start + 2] = "not-a-date".to_string();
            prop_assert_eq!(
                try_match_date_sequence(&non_date_component, start),
                None,
                "non-date component in layout {} must be rejected",
                layout_index,
            );

            for separator_offset in [1usize, 3] {
                let mut wrong_separator = tokens.clone();
                wrong_separator[start + separator_offset] = "-".to_string();
                prop_assert_eq!(
                    try_match_date_sequence(&wrong_separator, start),
                    None,
                    "wrong separator {} in layout {} must be rejected",
                    separator_offset,
                    layout_index,
                );
            }

            let mut wrong_order = tokens.clone();
            wrong_order.swap(start, start + 4);
            prop_assert_eq!(
                try_match_date_sequence(&wrong_order, start),
                None,
                "wrong token order in layout {} must be rejected",
                layout_index,
            );

            let mut wrong_opener = tokens.clone();
            wrong_opener[start].insert(0, invalid_opener);
            prop_assert_eq!(
                try_match_date_sequence(&wrong_opener, start),
                None,
                "wrong opener in layout {} must be rejected",
                layout_index,
            );
        }
    }

    #[test]
    fn try_match_date_sequence_rejects_generated_incomplete_dates(
        (
            mut date_tokens,
            opening_prefix,
            mut tokens,
            available_date_tokens,
        ) in (
            date_sequence_tokens_strategy(),
            opening_prefix_strategy(),
            preceding_tokens_strategy(),
            0usize..5,
        ),
    ) {
        date_tokens[0].insert_str(0, &opening_prefix);
        date_tokens.truncate(available_date_tokens);
        let start = tokens.len();
        tokens.extend(date_tokens);

        prop_assert_eq!(try_match_date_sequence(&tokens, start), None);
    }
}
