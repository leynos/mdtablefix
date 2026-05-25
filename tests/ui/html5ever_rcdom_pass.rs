//! Ensures `markup5ever_rcdom::RcDom` implements the `TreeSink` expected by
//! the active `html5ever` parser stack.

use html5ever::{driver::ParseOpts, parse_document, tendril::TendrilSink};
use markup5ever_rcdom::RcDom;

fn main() {
    let opts = ParseOpts::default();
    let dom: RcDom = parse_document(RcDom::default(), opts).one("<table></table>");

    let _document = dom.document;
}
