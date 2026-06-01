//! Unit and property tests for HTML table conversion helpers.
//!
//! These tests are compiled as a child module of `html`, so they exercise the
//! parent module's private parsing state and public conversion path without
//! keeping test-only code in the production source file.

use html5ever::{driver::ParseOpts, parse_document, tendril::TendrilSink};
use markup5ever_rcdom::RcDom;

use super::*;

#[test]
fn element_detection() {
    let dom: RcDom =
        parse_document(RcDom::default(), ParseOpts::default()).one("<table></table>".to_string());
    let html = dom.document.children.borrow()[0].clone();
    let body = html.children.borrow()[1].clone();
    let table = body.children.borrow()[0].clone();
    assert!(is_element(&table, "table"));
    assert!(is_element(&table, "TABLE"));
    assert!(!is_element(&table, "tr"));
}

#[test]
fn table_cell_detection() {
    let dom: RcDom = parse_document(RcDom::default(), ParseOpts::default())
        .one("<table><tr><th>a</th><td>b</td></tr></table>".to_string());
    let html = dom.document.children.borrow()[0].clone();
    let body = html.children.borrow()[1].clone();
    let table = body.children.borrow()[0].clone();
    let tbody = table.children.borrow()[0].clone();
    let tr = tbody.children.borrow()[0].clone();
    let th = tr.children.borrow()[0].clone();
    let td = tr.children.borrow()[1].clone();
    assert!(is_table_cell(&th));
    assert!(is_table_cell(&td));
}

#[test]
fn convert_html_tables_ignores_mid_line_table_tags() {
    let input = vec!["prefix <table><tr><td>Cell</td></tr></table>".to_string()];

    assert_eq!(convert_html_tables(&input), input);
}

mod proptest_tests {
    //! Property tests for HTML table conversion invariants.
    //!
    //! These generated cases complement the parent test module by checking
    //! `HtmlTableState` behaviour across varied open and close sequences.

    use proptest::prelude::*;

    use super::HtmlTableState;

    proptest! {
        #[test]
        fn html_table_state_depth_never_goes_negative(
            events in proptest::collection::vec(any::<bool>(), 1..=20),
        ) {
            let mut state = HtmlTableState::default();
            let mut out = Vec::new();
            for is_open in events {
                let line = if is_open { "<table>" } else { "</table>" };
                state.push_html_line(line, &mut out);
                // `depth` is `usize` and `saturating_sub` guards the close
                // path, so the count cannot wrap or panic. Once `depth`
                // returns to zero the buffer is flushed, so `in_html()`
                // must agree with `depth > 0` after every push.
                prop_assert_eq!(state.in_html(), state.depth > 0);
            }
        }
    }
}
