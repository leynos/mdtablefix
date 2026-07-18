//! Property tests for inline span helper date matching.

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

fn date_sequence_tokens_strategy() -> BoxedStrategy<Vec<String>> {
    prop_oneof![
        (
            ordinal_day_strategy(),
            whitespace_separator_strategy(),
            month_name_strategy(),
            whitespace_separator_strategy(),
            year_strategy(),
        )
            .prop_map(|(day, separator1, month, separator2, year)| {
                vec![day, separator1, month, separator2, year]
            }),
        (
            numeric_day_strategy(),
            whitespace_separator_strategy(),
            month_name_strategy(),
            whitespace_separator_strategy(),
            year_strategy(),
        )
            .prop_map(|(day, separator1, month, separator2, year)| {
                vec![day, separator1, month, separator2, year]
            }),
        (
            month_name_strategy(),
            whitespace_separator_strategy(),
            numeric_day_strategy(),
            whitespace_separator_strategy(),
            year_strategy(),
        )
            .prop_map(|(month, separator1, day, separator2, year)| {
                vec![month, separator1, day, separator2, year]
            }),
    ]
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
    fn try_match_date_sequence_accepts_prefixed_dates_at_generated_offsets(
        (
            mut date_tokens,
            opening_prefix,
            mut tokens,
        ) in (
            date_sequence_tokens_strategy(),
            opening_prefix_strategy(),
            preceding_tokens_strategy(),
        ),
    ) {
        date_tokens[0].insert_str(0, &opening_prefix);
        let start = tokens.len();
        tokens.extend(date_tokens);

        prop_assert_eq!(try_match_date_sequence(&tokens, start), Some(start + 5));
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
