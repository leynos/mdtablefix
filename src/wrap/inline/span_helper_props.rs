//! Property tests for inline span helper date matching.

use proptest::prelude::*;

use super::try_match_date_sequence;
use crate::wrap::inline::date_strategies::date_sequence_tokens_strategy;

fn prefixed_date_sequence_tokens_strategy() -> BoxedStrategy<Vec<String>> {
    (
        date_sequence_tokens_strategy(),
        prop_oneof![Just('('), Just('['), Just('"')],
    )
        .prop_map(|(mut tokens, opener)| {
            tokens[0].insert(0, opener);
            tokens
        })
        .boxed()
}

fn non_whitespace_separator_strategy() -> BoxedStrategy<String> {
    prop_oneof![Just("-"), Just("_"), Just("/"), Just(","), Just(".")]
        .prop_map(str::to_string)
        .boxed()
}

#[test]
fn prop_try_match_date_sequence_rejects_empty_slice() {
    assert!(try_match_date_sequence(&[], 0).is_none());
}

proptest! {
    #[test]
    fn prop_try_match_date_sequence_accepts_all_valid_patterns(
        tokens in date_sequence_tokens_strategy(),
    ) {
        prop_assert_eq!(try_match_date_sequence(&tokens, 0), Some(5));
    }

    #[test]
    fn prop_try_match_date_sequence_accepts_leading_opener_on_first_component(
        tokens in prefixed_date_sequence_tokens_strategy(),
    ) {
        prop_assert_eq!(try_match_date_sequence(&tokens, 0), Some(5));
    }

    #[test]
    fn prop_try_match_date_sequence_span_end_equals_start_plus_five(
        (date_tokens, offset) in (date_sequence_tokens_strategy(), 0usize..=8usize),
    ) {
        let mut tokens = vec!["filler".to_string(); offset];
        tokens.extend(date_tokens);
        prop_assert_eq!(try_match_date_sequence(&tokens, offset), Some(offset + 5));
    }

    #[test]
    fn prop_try_match_date_sequence_rejects_two_part(
        mut tokens in date_sequence_tokens_strategy(),
    ) {
        tokens.truncate(3);
        prop_assert!(try_match_date_sequence(&tokens, 0).is_none());
    }

    #[test]
    fn prop_try_match_date_sequence_rejects_non_whitespace_separator(
        (mut tokens, separator) in (
            date_sequence_tokens_strategy(),
            non_whitespace_separator_strategy(),
        ),
    ) {
        tokens[1] = separator;
        prop_assert!(try_match_date_sequence(&tokens, 0).is_none());
    }
}
