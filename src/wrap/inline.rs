//! Inline wrapping helpers that keep code spans intact.
//!
//! These functions operate on token streams so `wrap_text` can preserve
//! inline code, links, and trailing punctuation without reimplementing the
//! grouping logic in multiple places.
//!
//! The observer-aware orchestration that drives span grouping and line fitting
//! lives in the [`wrapping`] submodule; this module keeps the public wrapping
//! surface and the predicate re-exports shared across the inline submodules.

#[cfg(test)]
mod footnote_tests;
mod fragment;
mod month_names;
mod normalize;
#[cfg(test)]
mod observer_props;
mod postprocess;
mod predicates;
mod span_grouping;
mod span_helpers;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod wrapping;

// Shared predicate surface for the inline submodules. `fragment` and
// `normalize` reach these through `super::`, so they are re-exported here rather
// than imported directly from `predicates` in each module.
pub(in crate::wrap::inline) use predicates::{
    ends_with_footnote_ref,
    fragment_is_link,
    is_inline_code_token,
    is_opening_punct,
    is_trailing_punct,
    is_whitespace_token,
    looks_like_footnote_ref,
};
#[cfg(test)]
pub(super) use span_grouping::determine_token_span;
/// Re-exports the test-only helper that joins punctuation onto a prior code
/// line when `current` is empty.
#[cfg(test)]
pub(super) use test_support::attach_punctuation_to_previous_line;
// Public wrapping entry points, defined in `wrapping` and surfaced here so
// `paragraph` and the wrap test suites keep using `inline::…` paths.
pub(super) use wrapping::wrap_preserving_code;
// The observer-threaded entry point is only reached directly by the
// benchmark-only shims in `bench_internals`; production code goes through
// `wrap_preserving_code`, which wires up the `TracingObserver`.
#[cfg(feature = "bench-internals")]
pub(super) use wrapping::wrap_preserving_code_observed;

#[cfg(test)]
mod date_strategies;
