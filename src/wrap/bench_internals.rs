//! Benchmark-only shims and fixtures for the inline-wrapping observer boundary.
//!
//! This module is compiled only under the `bench-internals` feature so
//! [`benches/wrap_observer.rs`](../../benches/wrap_observer.rs) can measure the
//! observer-disabled (`ObserverHandle` is `None`) domain path against the
//! [`TracingObserver`](super::tracing_adapter::TracingObserver) path with no
//! DEBUG or TRACE subscriber installed. Nothing here is part of the public API
//! or compiled into production builds.
//!
//! The two wrapping shims expose the crate-internal
//! `wrap_preserving_code_observed` entry point so the benchmark can prove that,
//! with tracing disabled, the adapter adds no derived-payload work (Unicode
//! length counts and snippet truncation) beyond a single `tracing::enabled!`
//! branch per event.

use super::{inline::wrap_preserving_code_observed, tracing_adapter::TracingObserver};

/// Wrap width used by the benchmarks, matching the crate's default column
/// budget of 80 display columns.
pub const BENCH_WIDTH: usize = 80;

/// Wraps `text` with no observer attached (`ObserverHandle` is `None`).
///
/// This is the pure-domain baseline: no event is ever emitted, so no adapter
/// work runs at all.
#[must_use]
pub fn wrap_without_observer(text: &str, width: usize) -> Vec<String> {
    wrap_preserving_code_observed(text, width, &mut None)
}

/// Wraps `text` through [`TracingObserver`] while no DEBUG or TRACE subscriber
/// is installed, exercising the library boundary's disabled-tracing path.
#[must_use]
pub fn wrap_with_tracing_observer(text: &str, width: usize) -> Vec<String> {
    let mut observer = TracingObserver;
    wrap_preserving_code_observed(text, width, &mut Some(&mut observer))
}

/// Number of paragraphs in [`realistic_markdown_document`].
const DOCUMENT_PARAGRAPHS: usize = 60;

/// Number of clauses in [`large_inline_paragraph`].
const INLINE_CLAUSES: usize = 120;

/// Builds a large, realistic multi-line Markdown document that mixes ordinary
/// prose with inline links, inline code, and footnote references.
///
/// Paragraphs are separated by blank lines so `wrap_text` treats each as its
/// own reflow block. The content is deterministic: the same document is
/// produced on every call.
#[must_use]
pub fn realistic_markdown_document() -> Vec<String> {
    let mut lines = Vec::with_capacity(DOCUMENT_PARAGRAPHS * 2);
    for index in 0..DOCUMENT_PARAGRAPHS {
        lines.push(format!(
            "Paragraph {index}: the `wrap_text` routine reflows prose while \
             keeping the [textwrap {index}](https://example.com/crate/{index}) \
             link, the `inline-code-{index}` span, and the trailing footnote \
             reference intact.[^note{index}] Downstream tooling relies on the \
             stable column budget and content-free diagnostics."
        ));
        lines.push(String::new());
    }
    lines
}

/// Builds one large inline paragraph (a single logical line) that mixes prose,
/// inline links, inline code, and footnote references.
///
/// This drives the inline hot path — tokenization, span grouping, fragment
/// classification, and observer-event emission — repeatedly within a single
/// wrap call. The content is deterministic.
#[must_use]
pub fn large_inline_paragraph() -> String {
    let mut clauses = Vec::with_capacity(INLINE_CLAUSES);
    for index in 0..INLINE_CLAUSES {
        clauses.push(format!(
            "the `code-{index}` value links to \
             [reference {index}](https://example.com/path/{index}) and cites \
             the footnote [^fn{index}]"
        ));
    }
    clauses.join(", ")
}
