//! Traced-event tests for inline predicate helpers.
//!
//! These tests verify that instrumented predicate helpers emit TRACE events
//! when called, confirming that `#[tracing::instrument]` is active at the
//! declared log level.

use rstest::rstest;
use tracing_test::traced_test;

use super::{
    ends_with_footnote_ref,
    ends_with_hyphen_prefix,
    is_month_name,
    is_numeric_day,
    is_ordinal_day,
    is_year,
    looks_like_footnote_ref,
};

#[traced_test]
#[rstest(
    predicate,
    input,
    expected_log,
    case(looks_like_footnote_ref, "[^1]", "looks_like_footnote_ref"),
    case(ends_with_footnote_ref, "word.[^1]", "ends_with_footnote_ref"),
    case(ends_with_hyphen_prefix, "pre-", "ends_with_hyphen_prefix"),
    case(is_month_name, "January", "is_month_name"),
    case(is_ordinal_day, "25th", "is_ordinal_day"),
    case(is_numeric_day, "25", "is_numeric_day"),
    case(is_year, "2025", "is_year")
)]
#[test]
fn predicate_emits_trace_event(predicate: fn(&str) -> bool, input: &str, expected_log: &str) {
    let _ = predicate(input);
    assert!(logs_contain(expected_log));
    assert!(!logs_contain("token="));
}
