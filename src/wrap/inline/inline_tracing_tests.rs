//! Traced-event tests for inline span grouping helpers.
//!
//! These tests verify that date span grouping emits TRACE events from the
//! instrumentation boundary without exposing the helper outside the module.

use tracing_test::traced_test;

use super::date_token_span;

#[traced_test]
#[test]
fn date_token_span_emits_trace_event() {
    let tokens = [
        "25th".to_string(),
        " ".to_string(),
        "December".to_string(),
        " ".to_string(),
        "2025".to_string(),
    ];

    let _ = date_token_span(&tokens, 0);

    assert!(logs_contain("date_token_span"));
}
