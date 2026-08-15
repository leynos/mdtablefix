//! The observer port for inline-Markdown classification diagnostics.
//!
//! This module defines the boundary that keeps inline wrapping free of any
//! logging vendor. It owns three things:
//!
//! - [`Event`], the closed set of observable outcomes the domain reports — tokens parsed,
//!   predicates evaluated, spans grouped, fragments classified. [`FragmentKind`] and [`SpanKind`]
//!   are the classification vocabulary those events carry.
//! - [`Observer`], the port itself: a single `observe` method receiving one `Event`.
//! - [`ObserverHandle`], the optional borrowed handle threaded through the wrapping pipeline so any
//!   helper can report without owning an observer.
//!
//! # Flow
//!
//! Domain helpers in `crate::wrap::inline` and `crate::wrap::tokenize` take a
//! `&mut ObserverHandle<'_>` and, when it is `Some`, hand over an `Event`. They
//! never call `tracing`. The crate's production adapter,
//! [`TracingObserver`](super::tracing_adapter::TracingObserver), implements
//! `Observer` and turns each event into a level-gated `tracing` record; a
//! test-only [`NoOpObserver`] discards everything. Passing `None` runs the pure
//! domain path and emits nothing at all.
//!
//! # Cost and content rules
//!
//! Events carry only cheap borrowed data. Anything costlier than a copy — a
//! Unicode scan, for instance — is the observer's job, so a disabled adapter
//! pays only a branch. Observers must record content-free metadata: borrowed
//! token text exists so an observer can derive a length, never so it can log
//! the text. See `docs/developers-guide.md` and
//! `docs/adrs/0006-observer-boundary-for-tracing.md` for the ownership and
//! reuse policy governing this port.

use std::fmt;

/// Describes an inline fragment classification without binding it to a logger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FragmentKind {
    /// The fragment contains only whitespace.
    Whitespace,
    /// The fragment contains an inline code span.
    InlineCode,
    /// The fragment contains a Markdown link.
    Link,
    /// The fragment contains a GFM footnote reference.
    FootnoteRef,
    /// The fragment contains ordinary prose.
    Plain,
}

/// Marks how a grouped token span should behave during wrapping.
///
/// This lives beside [`FragmentKind`] because it is domain vocabulary carried
/// on [`Event`] values, not a detail of any one grouping helper.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpanKind {
    /// Treat the span as ordinary prose.
    General,
    /// Treat the span as an inline code sequence.
    Code,
    /// Treat the span as a Markdown link or image link.
    Link,
    /// Treat the span as a GitHub Flavoured Markdown footnote reference.
    FootnoteRef,
}

/// An observable outcome from inline parsing or classification.
///
/// Every variant carries only cheap, borrowed data (indices, flags, and string
/// slices). Any derived value that costs more than a copy — a Unicode scan such
/// as `chars().count()` — is deliberately left for the [`Observer`] to compute
/// so a disabled observer pays nothing. Emitters must not pre-compute those
/// values before handing an event to an observer.
///
/// Borrowed token text lets an observer derive bounded metadata such as a
/// character count. Observers must not record the text itself: diagnostics stay
/// content-free, as required by the security guidance in
/// `docs/developers-guide.md`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Event<'a> {
    /// A complete footnote reference was parsed.
    FootnoteReferenceParsed { token: &'a str },
    /// A Markdown link or image was parsed.
    LinkOrImageParsed { token: &'a str, is_image: bool },
    /// A footnote label could not be completed.
    FootnoteEndNotFound { start: usize, reason: &'static str },
    /// A footnote label span was recognised.
    FootnoteLabelRecognized {
        start: usize,
        end: usize,
        token: &'a str,
    },
    /// A footnote-reference predicate was evaluated.
    FootnoteRefChecked { token: &'a str, result: bool },
    /// A contiguous day–month–year run matched a recognised date pattern.
    ///
    /// `pattern` is a stable category name, never document text.
    DateSequenceMatched {
        start: usize,
        end: usize,
        pattern: &'static str,
    },
    /// A matched date sequence was grouped into one atomic wrap span.
    DateSequenceGrouped {
        start: usize,
        end: usize,
        width: usize,
    },
    /// Whitespace before a colon-suffixed footnote reference was considered for
    /// coupling into the current span.
    WhitespaceFootnoteCoupling {
        kind: SpanKind,
        token: &'a str,
        has_following_colon: bool,
        coupled: bool,
    },
    /// An adjacent footnote reference was considered for coupling into the
    /// current span.
    FootnoteReferenceCoupling {
        kind: SpanKind,
        token: &'a str,
        follows_space_before_colon: bool,
        coupled: bool,
    },
    /// A rendered fragment was classified.
    FragmentClassified { token: &'a str, kind: FragmentKind },
}

/// Receives domain-level classification events.
///
/// The `Observer` port is the sole boundary between inline-wrapping domain
/// logic and any diagnostics backend. Domain modules emit [`Event`] values and
/// never reference a logging vendor directly; an adapter such as
/// `TracingObserver` translates events into concrete log records. See
/// `docs/developers-guide.md` ("Inline classification observer boundary") for
/// the ownership and reuse policy governing this port.
pub(crate) trait Observer {
    /// Records one event.
    fn observe(&mut self, event: Event<'_>);
}

/// A borrowed, optional handle to an [`Observer`] threaded through the inline
/// wrapping pipeline.
///
/// Passing `&mut ObserverHandle<'_>` lets a caller opt out of observation with
/// `None` while keeping a single mutable observer borrow flowing through the
/// classification helpers without repeating the verbose option-of-trait-object
/// type at every signature.
pub(crate) type ObserverHandle<'a> = Option<&'a mut dyn Observer>;

/// Discards every event.
#[cfg(test)]
pub(crate) struct NoOpObserver;

#[cfg(test)]
impl Observer for NoOpObserver {
    fn observe(&mut self, _: Event<'_>) {}
}

impl fmt::Display for FragmentKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
