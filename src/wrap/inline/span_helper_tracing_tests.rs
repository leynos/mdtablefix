//! Traced-event tests for inline span helper instrumentation.

use tracing_test::traced_test;

use super::{date_token_span, try_match_date_sequence};
use crate::wrap::inline::determine_token_span;

fn date_tokens() -> [String; 5] {
    [
        "25th".to_string(),
        " ".to_string(),
        "December".to_string(),
        " ".to_string(),
        "2025".to_string(),
    ]
}

#[traced_test]
#[test]
fn try_match_date_sequence_emits_trace_event() {
    let tokens = date_tokens();
    let _ = try_match_date_sequence(&tokens, 0);
    assert!(logs_contain("try_match_date_sequence"));
}

#[traced_test]
#[test]
fn try_match_date_sequence_logs_matched_pattern() {
    let tokens = date_tokens();
    let _ = try_match_date_sequence(&tokens, 0);
    assert!(logs_contain("ordinal_day_month_year"));
}

#[traced_test]
#[test]
fn date_token_span_emits_trace_event() {
    let tokens = date_tokens();
    let _ = date_token_span(&tokens, 0);
    assert!(logs_contain("date_token_span"));
}

#[traced_test]
#[test]
fn grouping_boundary_logs_colon_footnote_whitespace_coupling() {
    let tokens = [
        "word".to_string(),
        " ".to_string(),
        "[^note]".to_string(),
        ":".to_string(),
    ];

    let _ = determine_token_span(&tokens, 0);
    assert!(logs_contain(
        "coupled whitespace before colon-suffixed footnote reference"
    ));
    assert!(logs_contain("span_kind=General"));
    assert!(logs_contain("token_length=7"));
    assert!(!logs_contain("[^note]"));
}

#[traced_test]
#[test]
fn grouping_boundary_logs_declined_colon_footnote_whitespace_coupling() {
    let tokens = ["word".to_string(), " ".to_string(), "[^note]".to_string()];

    let _ = determine_token_span(&tokens, 0);
    assert!(logs_contain(
        "error_category=\"footnote_colon_whitespace_coupling_declined\""
    ));
}

#[traced_test]
#[test]
fn grouping_boundary_logs_whitespace_colon_footnote_coupling() {
    let tokens = [
        "word".to_string(),
        " ".to_string(),
        "[^note]".to_string(),
        ":".to_string(),
    ];

    let _ = determine_token_span(&tokens, 0);

    assert!(logs_contain(
        "coupled colon-suffixed footnote reference after whitespace"
    ));
    assert!(logs_contain("span_kind=General"));
    assert!(logs_contain("token_length=7"));
    assert!(!logs_contain("[^note]"));
}

#[traced_test]
#[test]
fn grouping_boundary_logs_declined_footnote_context() {
    let tokens = ["word".to_string(), "[^note]".to_string()];

    let _ = determine_token_span(&tokens, 0);

    assert!(logs_contain(
        "error_category=\"footnote_coupling_context_mismatch\""
    ));
}
