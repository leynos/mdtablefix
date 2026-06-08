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

pub(crate) use source::{date_sequence_tokens_strategy, month_name_strategy, year_strategy};
