//! Proves the benchmark fixtures exercise links, inline code, and footnote
//! references, and that the public wrapping path preserves those constructs.
//!
//! These tests are content assertions only — they never assert on elapsed time
//! — and run under `make test` because that target enables all features,
//! including `bench-internals`.
#![cfg(feature = "bench-internals")]

use mdtablefix::{
    wrap::bench_internals::{
        BENCH_WIDTH,
        DOCUMENT_PARAGRAPHS,
        INLINE_CLAUSES,
        large_inline_paragraph,
        realistic_markdown_document,
    },
    wrap_text,
};

/// Asserts `haystack` contains a Markdown link, an inline code span, and a GFM
/// footnote reference.
fn assert_covers_link_code_footnote(haystack: &str, context: &str) {
    assert!(
        haystack.contains("]("),
        "{context} should contain a Markdown link"
    );
    assert!(
        haystack.contains('`'),
        "{context} should contain an inline code span"
    );
    assert!(
        haystack.contains("[^"),
        "{context} should contain a footnote reference"
    );
}

/// Asserts each of `constructs` appears verbatim in `haystack`.
///
/// Every generated index is checked so a fixture that degrades after the first
/// entry — or a wrap that splits a later construct — still fails.
fn assert_all_constructs_intact(haystack: &str, constructs: &[(usize, String)], context: &str) {
    for (index, construct) in constructs {
        assert!(
            haystack.contains(construct.as_str()),
            "{context}: construct {construct} for index {index} is missing or split"
        );
    }
}

/// Builds the code span, link, and footnote reference expected at `index` for
/// the given fixture naming scheme.
fn constructs_for(
    count: usize,
    code: fn(usize) -> String,
    link: fn(usize) -> String,
    footnote: fn(usize) -> String,
) -> Vec<(usize, String)> {
    (0..count)
        .flat_map(|index| [code(index), link(index), footnote(index)].map(move |c| (index, c)))
        .collect()
}

#[test]
fn realistic_document_fixture_covers_and_preserves_constructs() {
    let document = realistic_markdown_document();
    let source = document.join("\n");
    assert_covers_link_code_footnote(&source, "the document fixture");

    let constructs = constructs_for(
        DOCUMENT_PARAGRAPHS,
        |index| format!("`inline-code-{index}`"),
        |index| format!("(https://example.com/crate/{index})"),
        |index| format!("[^note{index}]"),
    );
    assert_all_constructs_intact(&source, &constructs, "the document fixture");

    let wrapped = wrap_text(&document, BENCH_WIDTH).join("\n");
    assert_covers_link_code_footnote(&wrapped, "the wrapped document");
    assert_all_constructs_intact(&wrapped, &constructs, "the wrapped document");
}

#[test]
fn large_inline_paragraph_fixture_covers_and_preserves_constructs() {
    let paragraph = large_inline_paragraph();
    assert_covers_link_code_footnote(&paragraph, "the inline paragraph fixture");

    let constructs = constructs_for(
        INLINE_CLAUSES,
        |index| format!("`code-{index}`"),
        |index| format!("(https://example.com/path/{index})"),
        |index| format!("[^fn{index}]"),
    );
    assert_all_constructs_intact(&paragraph, &constructs, "the inline paragraph fixture");

    let wrapped = wrap_text(&[paragraph], BENCH_WIDTH).join("\n");
    assert_covers_link_code_footnote(&wrapped, "the wrapped inline paragraph");
    assert_all_constructs_intact(&wrapped, &constructs, "the wrapped inline paragraph");
}
