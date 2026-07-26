//! Benchmarks for the inline-wrapping observer boundary.
//!
//! These benchmarks protect the performance invariant introduced with the
//! observer/tracing adapter (see `docs/adrs/0006-observer-boundary-for-tracing.md`):
//! with tracing disabled, `TracingObserver` must add no derived-payload work —
//! Unicode length counts or snippet truncation — beyond a single
//! `tracing::enabled!` branch per event. Comparing the `observer_none` and
//! `tracing_observer_disabled` inline cases makes any regression in that
//! invariant visible as a growing gap between the two.
//!
//! Run with:
//!
//! ```sh
//! cargo bench --features bench-internals --bench wrap_observer
//! ```

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use mdtablefix::{
    wrap::bench_internals::{
        BENCH_WIDTH,
        large_inline_paragraph,
        realistic_markdown_document,
        wrap_with_tracing_observer,
        wrap_without_observer,
    },
    wrap_text,
};

/// Benchmarks the public `wrap_text` path over a large realistic document.
///
/// `wrap_text` always drives the wrapping pipeline through `TracingObserver`,
/// so this measures the library boundary with no subscriber installed.
fn bench_public_document(c: &mut Criterion) {
    let document = realistic_markdown_document();
    c.bench_function("wrap_text_realistic_document", |b| {
        b.iter(|| wrap_text(black_box(&document), black_box(BENCH_WIDTH)));
    });
}

/// Benchmarks the inline hot path with the observer disabled (`None`) and with
/// `TracingObserver` while no subscriber is installed.
fn bench_inline_observer_paths(c: &mut Criterion) {
    let paragraph = large_inline_paragraph();
    let mut group = c.benchmark_group("inline_wrapping");
    group.bench_function("observer_none", |b| {
        b.iter(|| wrap_without_observer(black_box(&paragraph), black_box(BENCH_WIDTH)));
    });
    group.bench_function("tracing_observer_disabled", |b| {
        b.iter(|| wrap_with_tracing_observer(black_box(&paragraph), black_box(BENCH_WIDTH)));
    });
    group.finish();
}

criterion_group!(benches, bench_public_document, bench_inline_observer_paths);
criterion_main!(benches);
