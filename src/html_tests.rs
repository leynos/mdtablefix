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

    use std::rc::Rc;

    use html5ever::{driver::ParseOpts, parse_document, tendril::TendrilSink};
    use markup5ever_rcdom::{Handle, RcDom};
    use proptest::prelude::*;

    use super::{HtmlTableState, collect_matching, is_element};

    fn html_fragment_strategy() -> impl Strategy<Value = String> {
        (
            proptest::collection::vec(proptest::collection::vec(0usize..=4, 0..=6), 0..=4),
            0usize..=4,
        )
            .prop_map(|(tables, nested_depth)| {
                let nested_tables = (0..nested_depth).fold(String::new(), |nested, level| {
                    format!("<table><tr><td>level-{level}{nested}</td></tr></table>")
                });
                let mut html = tables.into_iter().fold(String::new(), |mut html, rows| {
                    html.push_str("<table>");
                    for cell_count in rows {
                        html.push_str("<tr>");
                        for index in 0..cell_count {
                            html.push_str("<td>cell-");
                            html.push_str(&index.to_string());
                            html.push_str("</td>");
                        }
                        html.push_str("</tr>");
                    }
                    html.push_str("</table>");
                    html
                });
                html.push_str(&nested_tables);
                html
            })
    }

    fn parse_html(source: String) -> RcDom {
        parse_document(RcDom::default(), ParseOpts::default()).one(source)
    }

    fn collect_tag(document: &Handle, tag: &'static str) -> Vec<Handle> {
        let mut matches = Vec::new();
        collect_matching(document, |node| is_element(node, tag), &mut matches);
        matches
    }

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

        #[test]
        fn html_table_state_buffers_until_all_nested_tables_close(
            nested_count in 0usize..=4,
        ) {
            let mut state = HtmlTableState::default();
            let mut out = Vec::new();
            let opens = "<table>".repeat(nested_count + 1);
            let closes = "</table>".repeat(nested_count);

            state.push_html_line(&opens, &mut out);
            prop_assert!(state.in_html());
            prop_assert!(out.is_empty());

            state.push_html_line(&closes, &mut out);
            prop_assert!(state.in_html());
            prop_assert!(out.is_empty());

            state.push_html_line("</table>", &mut out);
            prop_assert!(!state.in_html());
            prop_assert_eq!(state.depth, 0);
        }

        #[test]
        fn collect_matching_count_equals_source_tag_count(
            source in html_fragment_strategy(),
            tag in prop_oneof![Just("table"), Just("tr"), Just("td")],
        ) {
            let expected_count = source.matches(&format!("<{tag}")).count();
            let dom = parse_html(source);

            prop_assert_eq!(collect_tag(&dom.document, tag).len(), expected_count);
        }

        #[test]
        fn collect_matching_order_is_deterministic(
            source in html_fragment_strategy(),
            tag in prop_oneof![Just("table"), Just("tr"), Just("td")],
        ) {
            let dom = parse_html(source);
            let first = collect_tag(&dom.document, tag);
            let second = collect_tag(&dom.document, tag);

            prop_assert_eq!(first.len(), second.len());
            prop_assert!(first.iter().zip(&second).all(|(left, right)| Rc::ptr_eq(left, right)));
        }

        #[test]
        fn collect_matching_returns_empty_for_non_matching_documents(
            content in "[a-z ]{0,64}",
            tag in prop_oneof![Just("table"), Just("tr"), Just("td")],
        ) {
            let dom = parse_html(format!("<main><p>{content}</p></main>"));

            prop_assert!(collect_tag(&dom.document, tag).is_empty());
        }
    }
}
