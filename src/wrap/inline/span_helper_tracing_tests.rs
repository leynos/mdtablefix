//! Traced-event tests for inline span helper instrumentation.

use tracing_test::traced_test;

use super::{
    SpanKind,
    date_token_span,
    should_couple_whitespace,
    try_couple_footnote_reference,
    try_match_date_sequence,
};

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
fn should_couple_whitespace_logs_colon_footnote_coupling() {
    let footnote = "[^note]".to_string();
    let colon = ":".to_string();

    assert!(should_couple_whitespace(
        SpanKind::General,
        Some(&footnote),
        Some(&colon),
    ));
    assert!(logs_contain(
        "coupled whitespace before colon-suffixed footnote reference"
    ));
    assert!(logs_contain("span_kind=General"));
    assert!(logs_contain("token_length=7"));
    assert!(!logs_contain("[^note]"));
}

#[traced_test]
#[test]
fn should_couple_whitespace_logs_declined_colon_footnote_coupling() {
    let footnote = "[^note]".to_string();

    assert!(!should_couple_whitespace(
        SpanKind::General,
        Some(&footnote),
        None,
    ));
    assert!(logs_contain(
        "error_category=\"footnote_colon_whitespace_coupling_declined\""
    ));
}

#[traced_test]
#[test]
fn try_couple_footnote_reference_logs_whitespace_colon_coupling() {
    let tokens = [" ".to_string(), "[^note]".to_string(), ":".to_string()];
    let mut width = 0;

    let coupled = try_couple_footnote_reference(&tokens, 1, SpanKind::General, &mut width);

    assert!(coupled.is_some());
    assert!(logs_contain(
        "coupled colon-suffixed footnote reference after whitespace"
    ));
    assert!(logs_contain("span_kind=General"));
    assert!(logs_contain("token_length=7"));
    assert!(!logs_contain("[^note]"));
}

#[traced_test]
#[test]
fn try_couple_footnote_reference_logs_declined_context() {
    let tokens = ["word".to_string(), "[^note]".to_string()];
    let mut width = 0;

    let coupled = try_couple_footnote_reference(&tokens, 1, SpanKind::General, &mut width);

    assert!(coupled.is_none());
    assert!(logs_contain(
        "error_category=\"footnote_coupling_context_mismatch\""
    ));
}
