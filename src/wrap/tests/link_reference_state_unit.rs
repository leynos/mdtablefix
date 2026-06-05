//! Unit tests for link reference continuation state transitions.

use crate::wrap::link_reference::{LinkReferenceMatcher, LinkTitleWindow, LinkTitleWindowOutcome};

#[test]
fn non_destination_lines_are_not_url_continuations() {
    let matcher = LinkReferenceMatcher::production();
    for line in [
        " - item",
        "  * item",
        "  1. item",
        " > quote",
        " # heading",
        " plain prose",
        " https://example.com My Title",
    ] {
        assert!(!matcher.is_url_continuation_line(line));
    }
}

#[test]
fn markdown_prefixed_lines_after_bare_label_are_reprocessed() {
    let matcher = LinkReferenceMatcher::production();
    let mut window = LinkTitleWindow::AwaitingUrlContinuation;
    assert_eq!(
        window.observe_next_line(" - item", matcher),
        Some(LinkTitleWindowOutcome::Reprocess)
    );
    assert_eq!(window, LinkTitleWindow::Closed);
}

#[test]
fn bare_definition_opens_window() {
    let mut window = LinkTitleWindow::Closed;
    window.observe_bare_definition();
    assert_eq!(window, LinkTitleWindow::AwaitingStandaloneTitle);
}

#[test]
fn bare_label_opens_url_continuation_window() {
    let mut window = LinkTitleWindow::Closed;
    window.observe_bare_label();
    assert_eq!(window, LinkTitleWindow::AwaitingUrlContinuation);
}
