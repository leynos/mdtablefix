//! Snapshot test for fragment-classification tracing events.

use std::cell::RefCell;

use tracing_test::traced_test;

use super::InlineFragment;
use crate::wrap::tracing_snapshot_support::normalise_event_lines;

#[traced_test]
#[test]
fn snapshots_fragment_classified_event() {
    let captured = RefCell::new(String::new());
    let _ = InlineFragment::new("plain".to_string());
    logs_assert(|lines| {
        captured.replace(normalise_event_lines(lines, "fragment classified"));
        (!captured.borrow().is_empty())
            .then_some(())
            .ok_or_else(|| "expected fragment classification event".to_string())
    });
    let event = captured.into_inner();

    insta::with_settings!({prepend_module_to_snapshot => false}, {
        insta::assert_snapshot!("fragment-classified-event", event);
    });
}
