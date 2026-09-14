//! Compile-pass fixture: the footnote stages stay callable, one by one, at
//! their documented signatures.

fn main() {
    // Each stage is pinned by coercing the function item to a function pointer,
    // so a signature change breaks the build instead of passing unnoticed.
    let _inline: fn(&[String]) -> Vec<String> = mdtablefix::footnotes::convert_inline_footnotes;
    let _labels: fn(&[String]) -> Vec<String> = mdtablefix::footnotes::renumber_footnote_labels;
    let _definitions: fn(&[String]) -> Vec<String> =
        mdtablefix::footnotes::convert_footnote_definitions;
    let _whole: fn(&[String]) -> Vec<String> = mdtablefix::footnotes::convert_footnotes;
}
