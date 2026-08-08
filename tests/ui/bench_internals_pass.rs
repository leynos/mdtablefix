//! Compile-pass fixture: the feature-gated `wrap::bench_internals` surface
//! stays usable by a downstream crate that writes its own wrapping benchmarks
//! when `bench-internals` is enabled.
//!
//! This fixture covers the API surface only — that every item is reachable and
//! callable with the expected signature. The shims' behavioural contract is
//! asserted in `tests/bench_fixtures.rs`, which runs as an ordinary test rather
//! than inside the compile harness.

use mdtablefix::wrap::bench_internals::{
    BENCH_WIDTH,
    DOCUMENT_PARAGRAPHS,
    INLINE_CLAUSES,
    large_inline_paragraph,
    realistic_markdown_document,
    wrap_with_tracing_observer,
    wrap_without_observer,
};

fn main() {
    // The fixture builders are deterministic and describe their own size.
    assert_eq!(BENCH_WIDTH, 80);
    assert!(DOCUMENT_PARAGRAPHS > 0);
    assert!(INLINE_CLAUSES > 0);

    let document = realistic_markdown_document();
    assert_eq!(document, realistic_markdown_document());
    assert!(!document.is_empty());

    let paragraph = large_inline_paragraph();
    assert_eq!(paragraph, large_inline_paragraph());
    assert!(paragraph.contains("]("), "fixture covers Markdown links");
    assert!(paragraph.contains('`'), "fixture covers inline code");
    assert!(paragraph.contains("[^"), "fixture covers footnote references");

    // Both shims are reachable and return wrapped lines. Their behavioural
    // equivalence is asserted in `tests/bench_fixtures.rs`.
    assert!(!wrap_without_observer(&paragraph, BENCH_WIDTH).is_empty());
    assert!(!wrap_with_tracing_observer(&paragraph, BENCH_WIDTH).is_empty());
}
