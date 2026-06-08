# Architecture Decision Record (ADR) 0003: Treat prose dates as inline fragments

- Status: Accepted
- Date: 2026-06-08

## Context

`mdtablefix --wrap` delegates line fitting to `textwrap`, but it still decides
which Markdown-aware token runs are indivisible before calling the fitter.
Inline code spans, links, footnote references, opener punctuation, suffixes,
and trailing punctuation already use this span-grouping pipeline. Prose dates
such as `25th December 2025`, `19 March 2018`, and `July 4, 2008` were still
plain token runs, so greedy wrapping could split the day, month, and year even
when the full date would fit on one line.

## Decision

The implementation matches contiguous date-component tokens inside
`src/wrap/inline/span_helpers.rs` before line fitting, instead of adding a
TeX-style penalty system to the line fitter. This keeps date atomicity inside
the existing `InlineFragment` grouping model, while unsupported date forms
remain ordinary prose until explicitly added.

The matcher recognises exactly the supported day-month-year forms with
whitespace between components:

- ordinal day, month name, year;
- numeric day, month name, year;
- month name, numeric day, year.

Month names may be full or abbreviated, and numeric day tokens may carry a
trailing comma. Year tokens may carry trailing prose punctuation or closing
punctuation so sentence-final and parenthesised dates still form one span.
Leading opener punctuation on the first component is stripped by the
date-component predicates. If the resulting date span is wider than the
configured wrap width, it follows the same long-token fallback behaviour as
other atomic fragments.

## Consequences

- Date atomicity is implemented where other "do not break here" rules already
  live, so `textwrap::wrap_first_fit` remains an unmodified greedy fitter.
- The grouping decision is observable through TRACE-level instrumentation on
  the date matcher and the `date_token_span` boundary.
- Adding new date forms requires new span-helper patterns and predicate test
  coverage, not a redesign of line fitting.
