//! Traced-event tests for inline span-helper diagnostics.
//!
//! The grouping helpers no longer call `tracing` themselves; they emit domain
//! `Event` values that `TracingObserver` translates. These tests therefore
//! attach a `TracingObserver` and assert on the records it produces, keeping
//! the diagnostics contract covered across the observer boundary.

use std::cell::RefCell;

use rstest::{fixture, rstest};
use tracing_test::traced_test;

use super::{date_token_span, try_match_date_sequence};
use crate::wrap::{
    inline::span_grouping::determine_token_span_observed,
    tracing_adapter::TracingObserver,
    tracing_snapshot_support::normalise_event_lines,
};

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

/// Groups `tokens` from index 0 with a `TracingObserver` attached.
fn grouped_with_observer(tokens: &[String]) -> (usize, usize) {
    let mut observer = TracingObserver;
    determine_token_span_observed(tokens, 0, &mut Some(&mut observer))
}

#[traced_test]
#[rstest]
fn try_match_date_sequence_reports_matched_pattern(date_tokens: Vec<String>) {
    let mut observer = TracingObserver;
    // Observable result: all five date tokens form one sequence (exclusive end).
    assert_eq!(
        try_match_date_sequence(&date_tokens, 0, &mut Some(&mut observer)),
        Some(5)
    );
    assert!(logs_contain("matched date sequence"));
    assert!(logs_contain("ordinal_day_month_year"));
}

#[traced_test]
#[rstest]
fn date_token_span_reports_matched_sequence(date_tokens: Vec<String>) {
    let mut observer = TracingObserver;
    // Observable result: the span covers all five tokens with display width 18
    // ("25th December 2025"); no footnote coupling applies here.
    assert_eq!(
        date_token_span(&date_tokens, 0, &mut Some(&mut observer)),
        Some((5, 18))
    );
    assert!(logs_contain("matched date sequence"));
    assert!(logs_contain("start=0"));
    assert!(logs_contain("end=5"));
}

#[traced_test]
#[rstest]
#[case::whitespace("coupled whitespace before colon-suffixed footnote reference")]
#[case::reference("coupled colon-suffixed footnote reference after whitespace")]
fn grouping_boundary_logs_colon_footnote_coupling(
    colon_footnote_tokens: Vec<String>,
    #[case] expected_event: &str,
) {
    let _ = grouped_with_observer(&colon_footnote_tokens);
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
    let (end, width) = grouped_with_observer(&tokens);
    // Observable edge-case result: a declined coupling groups only the leading
    // "word" span (end == 1, never 0), leaving the footnote outside the span.
    assert_eq!(
        end, 1,
        "declined coupling must retain the leading word span"
    );
    let grouped = tokens[..end].join("");
    assert_eq!(grouped, "word");
    assert_eq!(width, 4);
    assert!(
        !grouped.contains("[^note]"),
        "declined coupling must not group the footnote reference",
    );
    assert!(logs_contain(&format!(
        "error_category=\"{error_category}\""
    )));
    assert!(!logs_contain("[^note]"));
}

#[traced_test]
#[rstest]
fn snapshots_grouped_date_sequence_event(date_tokens: Vec<String>) {
    let captured = RefCell::new(String::new());
    // Observable grouping result: the date tokens form one atomic span of
    // width 18. The snapshot below supplements this behavioural assertion.
    let (end, width) = grouped_with_observer(&date_tokens);
    assert_eq!(date_tokens[..end].join(""), "25th December 2025");
    assert_eq!(width, 18);
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
    // Observable grouping result backing the supplementary snapshot.
    let (end, width) = grouped_with_observer(&date_tokens);
    assert_eq!(date_tokens[..end].join(""), "25th December 2025");
    assert_eq!(width, 18);
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
