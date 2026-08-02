//! Snapshot tests for parsing tracing events.

use std::cell::RefCell;

use tracing_test::traced_test;

use super::parse_link_or_image;
use crate::wrap::tracing_snapshot_support::normalise_event_lines;

#[traced_test]
#[test]
fn snapshots_footnote_reference_parsed_event() {
    let captured = RefCell::new(String::new());
    // Observable parse result: the footnote reference is kept atomic and the
    // cursor advances past it. The snapshot is supplementary.
    let (token, idx) = parse_link_or_image("[^4] tail", 0);
    assert_eq!(token, "[^4]");
    assert_eq!(idx, token.len());
    logs_assert(|lines| {
        captured.replace(normalise_event_lines(lines, "footnote reference parsed"));
        (!captured.borrow().is_empty())
            .then_some(())
            .ok_or_else(|| "expected footnote reference parsed event".to_string())
    });
    let event = captured.into_inner();

    insta::with_settings!({prepend_module_to_snapshot => false}, {
        insta::assert_snapshot!("footnote-reference-parsed-event", event);
    });
}

#[traced_test]
#[test]
fn snapshots_link_or_image_parsed_event() {
    let captured = RefCell::new(String::new());
    // Observable parse result: the whole link is kept atomic. Supplementary
    // snapshot follows.
    let (token, idx) = parse_link_or_image("[link](url)", 0);
    assert_eq!(token, "[link](url)");
    assert_eq!(idx, token.len());
    logs_assert(|lines| {
        captured.replace(normalise_event_lines(lines, "link or image parsed"));
        (!captured.borrow().is_empty())
            .then_some(())
            .ok_or_else(|| "expected link or image parsed event".to_string())
    });
    let event = captured.into_inner();

    insta::with_settings!({prepend_module_to_snapshot => false}, {
        insta::assert_snapshot!("link-or-image-parsed-event", event);
    });
}
