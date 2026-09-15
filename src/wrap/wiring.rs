//! Composition point wiring the inline-wrapping domain to its tracing adapter.
//!
//! The domain under [`inline`](super::inline) and [`tokenize`](super::tokenize)
//! depends only on the [`Observer`](super::observer::Observer) port: it reports
//! `Event` values and never names a logging vendor. Something must still choose
//! the concrete adapter, and this module is that something. Keeping the choice
//! here rather than inside the wrapping helpers preserves a checkable
//! invariant — no module under `wrap/inline/` or `wrap/tokenize/` mentions
//! `tracing` or `TracingObserver` outside its own tests.
//!
//! See [ADR 0012](../../docs/adrs/0012-observer-boundary-for-tracing.md).

use super::{inline::wrap_preserving_code_observed, tracing_adapter::TracingObserver};

/// Wraps inline Markdown `text` without splitting code spans or links,
/// reporting classification events through the crate's tracing adapter.
///
/// This is the production entry point the paragraph-wrapping helpers call.
/// Wrapping behaviour and the meaning of `width` are exactly those of
/// [`wrap_preserving_code_observed`]; the only thing added here is the choice of
/// adapter, so callers never construct an observer themselves and every wrapped
/// paragraph crosses the same boundary. With no DEBUG or TRACE subscriber
/// installed the adapter's level gates suppress all derived work.
pub(in crate::wrap) fn wrap_preserving_code(text: &str, width: usize) -> Vec<String> {
    let mut observer = TracingObserver;
    wrap_preserving_code_observed(text, width, &mut Some(&mut observer))
}

#[cfg(test)]
#[path = "wiring_tracing_tests.rs"]
mod wiring_tracing_tests;
