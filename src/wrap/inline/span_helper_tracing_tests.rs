//! Traced-event tests for inline span helper instrumentation.

use std::cell::RefCell;

use rstest::{fixture, rstest};
use tracing_test::traced_test;

use super::{date_token_span, try_match_date_sequence};
use crate::wrap::{inline::determine_token_span, tracing_snapshot_support::normalise_event_lines};

#[fixture]
fn date_tokens() -> Vec<String> {
    vec![
        "25th".to_string(),
        " ".to_string(),
        "December".to_string(),
        " ".to_string(),
        "2025".to_string(),
    ]
}

#[fixture]
fn colon_footnote_tokens() -> Vec<String> {
    vec![
        "word".to_string(),
        " ".to_string(),
        "[^note]".to_string(),
        ":".to_string(),
    ]
}

#[traced_test]
#[rstest]
fn try_match_date_sequence_emits_trace_event(date_tokens: Vec<String>) {
    let _ = try_match_date_sequence(&date_tokens, 0);
    assert!(logs_contain("try_match_date_sequence"));
}

#[traced_test]
#[rstest]
fn try_match_date_sequence_logs_matched_pattern(date_tokens: Vec<String>) {
    let _ = try_match_date_sequence(&date_tokens, 0);
    assert!(logs_contain("ordinal_day_month_year"));
}

#[traced_test]
#[rstest]
fn date_token_span_emits_trace_event(date_tokens: Vec<String>) {
    let _ = date_token_span(&date_tokens, 0);
    assert!(logs_contain("date_token_span"));
}

#[traced_test]
#[rstest]
#[case::whitespace("coupled whitespace before colon-suffixed footnote reference")]
#[case::reference("coupled colon-suffixed footnote reference after whitespace")]
fn grouping_boundary_logs_colon_footnote_coupling(
    colon_footnote_tokens: Vec<String>,
    #[case] expected_event: &str,
) {
    let _ = determine_token_span(&colon_footnote_tokens, 0);
    assert!(logs_contain(expected_event));
    assert!(logs_contain("span_kind=General"));
    assert!(logs_contain("token_length=7"));
    assert!(!logs_contain("[^note]"));
}

#[traced_test]
#[rstest]
#[case::missing_colon(
    &["word", " ", "[^note]"],
    "footnote_colon_whitespace_coupling_declined"
)]
#[case::context_mismatch(
    &["word", "[^note]"],
    "footnote_coupling_context_mismatch"
)]
fn grouping_boundary_logs_declined_footnote_coupling(
    #[case] token_text: &[&str],
    #[case] error_category: &str,
) {
    let tokens = token_text
        .iter()
        .map(|token| (*token).to_string())
        .collect::<Vec<_>>();
    let _ = determine_token_span(&tokens, 0);
    assert!(logs_contain(&format!(
        "error_category=\"{error_category}\""
    )));
    assert!(!logs_contain("[^note]"));
}

#[traced_test]
#[rstest]
fn snapshots_grouped_date_sequence_event(date_tokens: Vec<String>) {
    let captured = RefCell::new(String::new());
    let _ = determine_token_span(&date_tokens, 0);
    logs_assert(|lines| {
        captured.replace(normalise_event_lines(
            lines,
            "determine_token_span grouped date sequence",
        ));
        (!captured.borrow().is_empty())
            .then_some(())
            .ok_or_else(|| "expected grouped date sequence event".to_string())
    });
    let event = captured.into_inner();

    insta::with_settings!({prepend_module_to_snapshot => false}, {
        insta::assert_snapshot!("determine-token-span-grouped-date-sequence-event", event);
    });
}

#[traced_test]
#[rstest]
fn snapshots_matched_date_sequence_event(date_tokens: Vec<String>) {
    let captured = RefCell::new(String::new());
    let _ = determine_token_span(&date_tokens, 0);
    logs_assert(|lines| {
        captured.replace(normalise_event_lines(lines, "matched date sequence"));
        (!captured.borrow().is_empty())
            .then_some(())
            .ok_or_else(|| "expected matched date sequence event".to_string())
    });
    let event = captured.into_inner();

    insta::with_settings!({prepend_module_to_snapshot => false}, {
        insta::assert_snapshot!("matched-date-sequence-event", event);
    });
}
