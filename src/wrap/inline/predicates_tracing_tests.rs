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

// Exercises `TracingObserver` with no subscriber active — the configuration
// production callers use.
//
// The no-subscriber state is established explicitly rather than by omitting
// `#[traced_test]`. `tracing_test` installs a *global* dispatcher behind a
// `Once`, so a sibling traced test anywhere in this binary leaves it installed
// for the rest of the process; simply not annotating this test would leave what
// it exercises up to test ordering. A thread-local `NoSubscriber` takes
// precedence over that global, making the configuration deterministic.
#[test]
fn footnote_ref_check_with_observer_but_no_subscriber_returns_result() {
    tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
        let mut observer = TracingObserver;
        let mut handle = Some(&mut observer as &mut dyn crate::wrap::observer::Observer);
        assert!(looks_like_footnote_ref("[^1]", &mut handle));
        assert!(!looks_like_footnote_ref("plain", &mut handle));
        assert!(ends_with_footnote_ref("word.[^1]", &mut handle));
    });
}
