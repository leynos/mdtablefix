# Architecture Decision Record (ADR) 0006: Observer boundary for tracing

- Status: Accepted
- Date: 2026-07-26

## Context

The inline-wrapping domain logic under `src/wrap/tokenize/` and
`src/wrap/inline/` previously called `tracing` macros and
`#[tracing::instrument]` attributes directly to record classification
outcomes: fragment kinds, parsed link and footnote token lengths, footnote
label spans, and matched date sequences. This coupled parsing and
classification helpers — code whose correctness is independent of any
logging vendor — to a specific observability crate. It also pushed
`tracing::enabled!` level gates and derived-value computation (Unicode
`chars().count()` scans) into the same functions
that decide fragment boundaries, making it harder to unit test the domain
logic without a `tracing` subscriber and harder to change logging behaviour
without touching parsing code.

Issue #309 asked for vendor-specific tracing to be isolated from
inline-wrapping domain logic behind an adapter boundary, consistent with the
abstraction/port/helper policy in `AGENTS.md`.

## Decision

For inline-wrapping domain code that emits diagnostics, an `Observer` port,
not direct `tracing` calls, is the mechanism.

The `Observer` trait, its `Event` enum, and the `ObserverHandle<'a>` alias
(`Option<&'a mut dyn Observer>`) live in `src/wrap/observer.rs`. Domain
helpers under `src/wrap/tokenize/` and `src/wrap/inline/` accept
`&mut ObserverHandle<'_>` and call `observer.observe(event)` at the same
points where they previously called `tracing` macros; they no longer import
`tracing` or carry `#[tracing::instrument]` attributes. Every `Event` variant
carries only cheap, borrowed data — indices, flags, and `&str` slices — so
constructing an event costs no more than a few copies.

`TracingObserver`, the crate's single `Observer` implementation, lives in
`src/wrap/tracing_adapter.rs` and translates each `Event` into the crate's
existing `tracing` records. It owns every vendor-specific concern: the
`tracing::enabled!` level gate for each event and any derived value that
costs more than a copy, such as the `chars().count()` used to report
`token_length`. Diagnostics are metadata-only: the adapter derives bounded
values from the borrowed token text but never records the text itself, so no
raw document content reaches a subscriber. A `#[cfg(test)]`-only
`NoOpObserver` in `observer.rs` discards every event for tests that need an
`ObserverHandle` without a subscriber.

New diagnostics needs are met by adding an `Event` variant and a matching arm
in `TracingObserver::observe`, not by importing `tracing` into a domain
module or by adding a second adapter alongside `TracingObserver`.

## Consequences

- Domain parsing and classification helpers can be unit tested, including
  with property tests, without installing a `tracing` subscriber; a test can
  pass `None` or a fixture `Observer` in place of `TracingObserver`.
- All `tracing` field names, message strings, and levels for inline
  classification are defined in one file, `src/wrap/tracing_adapter.rs`,
  rather than scattered across `src/wrap/tokenize/` and `src/wrap/inline/`.
  Changing a message or promoting an event from `trace!` to `debug!` no
  longer risks touching parsing logic.
- Because `Event` variants carry only borrowed data and the adapter performs
  level-gating before any derived computation, a disabled subscriber still
  pays only the cost of constructing a borrowed enum value and evaluating one
  `tracing::enabled!` branch per event, matching the performance discipline
  already required of the `tracing` calls it replaced.
- The trade-off is one layer of indirection: reading what a domain helper
  logs now requires following the `Event` variant it emits to its match arm
  in `TracingObserver`, rather than reading a `tracing` macro call inline.
  `parse_rows` in `src/reflow.rs`, which is outside the inline-wrapping
  domain this port covers, keeps its existing `#[tracing::instrument]`
  attribute and is not affected by this decision.
