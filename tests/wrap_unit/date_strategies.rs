//! Integration-test access to the shared inline date strategies.
//!
//! The authoritative strategy implementations live under `src/wrap/inline/`
//! so predicate unit tests and wrap integration tests exercise the same input
//! shapes. This wrapper supplies the sibling month-name module expected by
//! that source file and re-exports only the helpers used by `dates_prop`.

#[path = "../../src/wrap/inline/month_names.rs"]
mod month_names;
#[path = "../../src/wrap/inline/date_strategies.rs"]
mod source;

use proptest::prelude::*;
pub(crate) use source::{month_name_strategy, year_strategy};

/// Generates tokenized date sequences for the integration properties.
pub(crate) fn date_sequence_tokens_strategy() -> BoxedStrategy<Vec<String>> {
    prop_oneof![
        (
            source::ordinal_day_strategy(),
            month_name_strategy(),
            year_strategy()
        )
            .prop_map(|(day, month, year)| vec![day, " ".into(), month, " ".into(), year]),
        (
            source::numeric_day_strategy(),
            month_name_strategy(),
            year_strategy()
        )
            .prop_map(|(day, month, year)| vec![day, " ".into(), month, " ".into(), year]),
        (
            month_name_strategy(),
            source::numeric_day_strategy(),
            year_strategy()
        )
            .prop_map(|(month, day, year)| vec![month, " ".into(), day, " ".into(), year]),
    ]
    .boxed()
}
