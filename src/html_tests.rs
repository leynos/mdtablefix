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
    use markup5ever_rcdom::{Handle, NodeData, RcDom};
    use proptest::prelude::*;

    use super::{HtmlTableState, collect_matching, is_element};

    /// Holds generated HTML and its independently constructed pre-order labels.
    #[derive(Debug)]
    struct HtmlFragment {
        source: String,
        pre_order_labels: Vec<(&'static str, String)>,
    }

    impl HtmlFragment {
        /// Returns the pre-order labels expected for elements with `tag`.
        fn labels_for(&self, tag: &str) -> Vec<String> {
            self.pre_order_labels
                .iter()
                .filter(|(element_tag, _)| *element_tag == tag)
                .map(|(_, label)| label.clone())
                .collect()
        }
    }

    /// Builds small sibling and nested table fragments with pre-order labels.
    fn html_fragment_strategy() -> impl Strategy<Value = HtmlFragment> {
        (
            proptest::collection::vec(proptest::collection::vec(0usize..=4, 0..=6), 0..=4),
            0usize..=4,
        )
            .prop_map(|(tables, nested_depth)| {
                let mut pre_order_labels = Vec::new();
                let mut next_order = 0;
                let mut html = tables.into_iter().fold(String::new(), |mut html, rows| {
                    append_opening_tag(&mut html, &mut pre_order_labels, &mut next_order, "table");
                    for cell_count in rows {
                        append_opening_tag(&mut html, &mut pre_order_labels, &mut next_order, "tr");
                        for index in 0..cell_count {
                            append_opening_tag(
                                &mut html,
                                &mut pre_order_labels,
                                &mut next_order,
                                "td",
                            );
                            html.push_str("cell-");
                            html.push_str(&index.to_string());
                            html.push_str("</td>");
                        }
                        html.push_str("</tr>");
                    }
                    html.push_str("</table>");
                    html
                });
                for _ in 0..nested_depth {
                    append_opening_tag(&mut html, &mut pre_order_labels, &mut next_order, "table");
                    append_opening_tag(&mut html, &mut pre_order_labels, &mut next_order, "tr");
                    append_opening_tag(&mut html, &mut pre_order_labels, &mut next_order, "td");
                }
                html.push_str("nested");
                for _ in 0..nested_depth {
                    html.push_str("</td></tr></table>");
                }

                HtmlFragment {
                    source: html,
                    pre_order_labels,
                }
            })
    }

    /// Appends a labelled opening tag and records its expected pre-order position.
    fn append_opening_tag(
        html: &mut String,
        pre_order_labels: &mut Vec<(&'static str, String)>,
        next_order: &mut usize,
        tag: &'static str,
    ) {
        let label = format!("{tag}-{next_order}");
        html.push('<');
        html.push_str(tag);
        html.push_str(" data-order=\"");
        html.push_str(&label);
        html.push_str("\">");
        pre_order_labels.push((tag, label));
        *next_order += 1;
    }

    /// Parses generated HTML into the DOM representation used by the walker.
    fn parse_html(source: String) -> RcDom {
        parse_document(RcDom::default(), ParseOpts::default()).one(source)
    }

    /// Collects all nodes matching `tag` from the parsed document.
    fn collect_tag(document: &Handle, tag: &'static str) -> Vec<Handle> {
        let mut matches = Vec::new();
        collect_matching(document, |node| is_element(node, tag), &mut matches);
        matches
    }

    /// Returns the generated ordering label for an element when it has one.
    fn order_label(handle: &Handle) -> Option<String> {
        let NodeData::Element { attrs, .. } = &handle.data else {
            return None;
        };

        attrs
            .borrow()
            .iter()
            .find(|attribute| attribute.name.local.as_ref() == "data-order")
            .map(|attribute| attribute.value.to_string())
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
            fragment in html_fragment_strategy(),
            tag in prop_oneof![Just("table"), Just("tr"), Just("td")],
        ) {
            let expected_count = fragment.source.matches(&format!("<{tag}")).count();
            let dom = parse_html(fragment.source);

            prop_assert_eq!(collect_tag(&dom.document, tag).len(), expected_count);
        }

        #[test]
        fn collect_matching_order_is_deterministic(
            fragment in html_fragment_strategy(),
            tag in prop_oneof![Just("table"), Just("tr"), Just("td")],
        ) {
            let expected_labels = fragment.labels_for(tag);
            let dom = parse_html(fragment.source);
            let first = collect_tag(&dom.document, tag);
            let second = collect_tag(&dom.document, tag);

            prop_assert_eq!(first.len(), second.len());
            prop_assert!(first.iter().zip(&second).all(|(left, right)| Rc::ptr_eq(left, right)));
            let actual_labels = first.iter().map(order_label).collect::<Vec<_>>();
            prop_assert_eq!(actual_labels, expected_labels.into_iter().map(Some).collect::<Vec<_>>());
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
