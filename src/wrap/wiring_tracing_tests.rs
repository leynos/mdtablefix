//! Traced-event tests for the production wrapping path.
//!
//! The other traced tests in this crate drive one helper at a time with a
//! hand-attached observer. These drive the public [`wrap_text`] entry point
//! with a real subscriber installed, so they fail if the adapter is ever
//! unwired from the path callers actually take — something a test that
//! constructs its own observer cannot detect.
//!
//! They also pin the security claim ADR 0012 makes: the adapter records
//! content-free scalars and never the token text. The shared fixture below
//! carries deliberately distinctive link, path, and footnote labels so their
//! absence from the log is meaningful rather than coincidental.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use rstest::{fixture, rstest};
// Wrapper over `tracing_test::traced_test`; see `test_macros` for why.
use test_macros::traced_test;
use tracing::{Metadata, span};

use crate::wrap_text;

/// Markdown carrying a link and a footnote whose labels appear nowhere else.
///
/// Shared so the reporting test and the security test cannot drift apart: the
/// second asserts these exact labels never reach a subscriber, which only means
/// anything if the first proved the same input does produce events.
#[fixture]
fn distinctive_markdown() -> &'static str {
    "prose [qwlabel](https://example.com/qwpath) then a note[^qwnote] and some trailing words to \
     make the paragraph wrap across a line boundary."
}

/// The labels the fixture embeds, none of which may reach a subscriber.
const DISTINCTIVE_LABELS: [&str; 3] = ["qwlabel", "qwpath", "qwnote"];

/// Wraps `text` as a single line through the public entry point.
fn wrap_line(text: &str) -> Vec<String> { wrap_text(&[text.to_string()], 80) }

/// A subscriber that enables every callsite and counts the events it receives.
///
/// The adapter gates its work behind `tracing::enabled!`, which only opens when
/// a subscriber says yes, so installing this is what makes the observed path
/// actually run. The count lets a test prove the subscriber was live rather
/// than assume it — without that, a test comparing observed against unobserved
/// output would pass even if no subscriber had been installed at all.
#[derive(Clone, Default)]
struct CountingSubscriber {
    events: Arc<AtomicUsize>,
}

impl tracing::Subscriber for CountingSubscriber {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool { true }

    fn new_span(&self, _span: &span::Attributes<'_>) -> span::Id { span::Id::from_u64(1) }

    fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {}

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}

    fn event(&self, _event: &tracing::Event<'_>) { self.events.fetch_add(1, Ordering::Relaxed); }

    fn enter(&self, _span: &span::Id) {}

    fn exit(&self, _span: &span::Id) {}
}

#[traced_test]
#[rstest]
fn production_path_reports_link_and_footnote_events(distinctive_markdown: &'static str) {
    let lines = wrap_line(distinctive_markdown);
    assert!(!lines.is_empty(), "wrapping produced no output");

    assert!(
        logs_contain("link or image parsed"),
        "the production path reported no link event"
    );
    assert!(
        logs_contain("footnote reference parsed"),
        "the production path reported no footnote event"
    );
    assert!(
        logs_contain("fragment classified"),
        "the production path reported no fragment classification"
    );
    assert!(
        logs_contain("token_length="),
        "events carried no token_length scalar"
    );
}

#[traced_test]
#[rstest]
fn production_path_never_logs_document_text(distinctive_markdown: &'static str) {
    let _ = wrap_line(distinctive_markdown);

    // The events fired at all, so the absence assertions below are meaningful.
    assert!(logs_contain("fragment classified"));

    for secret in DISTINCTIVE_LABELS {
        assert!(
            !logs_contain(secret),
            "raw document text {secret:?} reached a subscriber"
        );
    }
}

/// The public path must wrap identically whether or not a subscriber is
/// installed, since the observer is a diagnostics channel and never a
/// participant in layout.
///
/// Both dispatch modes are established explicitly rather than left to whatever
/// is ambient: one call runs under [`CountingSubscriber`], which enables every
/// callsite so the adapter does its derived work, and the other under
/// `NoSubscriber`, so neither depends on test ordering. The event count is
/// asserted first because without it this test would still pass if no
/// subscriber had been installed — comparing two identical unobserved runs.
#[rstest]
fn production_path_output_does_not_depend_on_subscriber(distinctive_markdown: &'static str) {
    let unobserved =
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            wrap_line(distinctive_markdown)
        });

    let subscriber = CountingSubscriber::default();
    let events = Arc::clone(&subscriber.events);
    let observed =
        tracing::subscriber::with_default(subscriber, || wrap_line(distinctive_markdown));

    assert!(
        events.load(Ordering::Relaxed) > 0,
        "the subscriber recorded no events, so this comparison proves nothing"
    );
    assert_eq!(
        unobserved, observed,
        "installing a subscriber changed the wrapped output"
    );
}
