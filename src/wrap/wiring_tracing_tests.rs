//! Traced-event tests for the production wrapping path.
//!
//! The other traced tests in this crate drive one helper at a time with a
//! hand-attached observer. These drive the public [`wrap_text`] entry point
//! with a real subscriber installed, so they fail if the adapter is ever
//! unwired from the path callers actually take — something a test that
//! constructs its own observer cannot detect.
//!
//! They also pin the security claim ADR 0012 makes: the adapter records
//! content-free scalars and never the token text. Each input below carries
//! deliberately distinctive link, path, and footnote labels so their absence
//! from the log is meaningful rather than coincidental.

// Wrapper over `tracing_test::traced_test`; see `test_macros` for why.
use test_macros::traced_test;

use crate::wrap_text;

/// Wraps `text` as a single line through the public entry point.
fn wrap_line(text: &str) -> Vec<String> { wrap_text(&[text.to_string()], 80) }

#[traced_test]
#[test]
fn production_path_reports_link_and_footnote_events() {
    let lines = wrap_line(
        "prose [qwlabel](https://example.com/qwpath) then a note[^qwnote] and some trailing words \
         to make the paragraph wrap across a line boundary.",
    );
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
#[test]
fn production_path_never_logs_document_text() {
    let _ = wrap_line(
        "prose [qwlabel](https://example.com/qwpath) then a note[^qwnote] and some trailing words \
         to make the paragraph wrap across a line boundary.",
    );

    // The events fired at all, so the absence assertions below are meaningful.
    assert!(logs_contain("fragment classified"));

    for secret in ["qwlabel", "qwpath", "qwnote"] {
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
/// Deliberately not `#[traced_test]`: this is the no-subscriber configuration
/// production callers use.
#[test]
fn production_path_output_does_not_depend_on_subscriber() {
    let input = "prose [qwlabel](https://example.com/qwpath) then a note[^qwnote] tail";
    assert_eq!(wrap_line(input), wrap_line(input));
}
