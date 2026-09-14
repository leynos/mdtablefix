//! Traced-event tests for the footnote-reference predicates.
//!
//! Verify that `looks_like_footnote_ref` and `ends_with_footnote_ref` emit
//! the TRACE `footnote reference checked` event through the tracing adapter,
//! including its `token_length` and `result` fields, and that the raw token
//! text never reaches the log.

// Wrapper over `tracing_test::traced_test`; see `test_macros` for why.

use test_macros::traced_test;

use super::{ends_with_footnote_ref, looks_like_footnote_ref};
use crate::wrap::tracing_adapter::TracingObserver;

#[traced_test]
#[test]
fn looks_like_footnote_ref_logs_positive_check() {
    let mut observer = TracingObserver;
    assert!(looks_like_footnote_ref("[^note]", &mut Some(&mut observer)));
    assert!(logs_contain("footnote reference checked"));
    assert!(logs_contain("token_length=7"));
    assert!(logs_contain("result=true"));
    assert!(!logs_contain("[^note]"));
}

#[traced_test]
#[test]
fn looks_like_footnote_ref_logs_negative_check() {
    let mut observer = TracingObserver;
    assert!(!looks_like_footnote_ref("plain", &mut Some(&mut observer)));
    assert!(logs_contain("footnote reference checked"));
    assert!(logs_contain("token_length=5"));
    assert!(logs_contain("result=false"));
}

#[traced_test]
#[test]
fn ends_with_footnote_ref_logs_positive_check() {
    let mut observer = TracingObserver;
    assert!(ends_with_footnote_ref(
        "word.[^1]",
        &mut Some(&mut observer)
    ));
    assert!(logs_contain("footnote reference checked"));
    assert!(logs_contain("token_length=4"));
    assert!(logs_contain("result=true"));
    assert!(!logs_contain("[^1]"));
}

#[test]
fn footnote_ref_check_without_observer_returns_result() {
    assert!(looks_like_footnote_ref("[^1]", &mut None));
    assert!(!looks_like_footnote_ref("plain", &mut None));
}

// Deliberately not `#[traced_test]`: `tracing_test` installs its subscriber
// only for the test it decorates, so this test exercises `TracingObserver`
// with no subscriber active — the configuration production callers use.
#[test]
fn footnote_ref_check_with_observer_but_no_subscriber_returns_result() {
    let mut observer = TracingObserver;
    let mut handle = Some(&mut observer as &mut dyn crate::wrap::observer::Observer);
    assert!(looks_like_footnote_ref("[^1]", &mut handle));
    assert!(!looks_like_footnote_ref("plain", &mut handle));
    assert!(ends_with_footnote_ref("word.[^1]", &mut handle));
}
