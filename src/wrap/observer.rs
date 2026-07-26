//! Domain events emitted while classifying inline Markdown.

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

/// An observable outcome from inline parsing or classification.
///
/// Every variant carries only cheap, borrowed data (indices, flags, and string
/// slices). Any derived value that costs more than a copy — a Unicode scan such
/// as `chars().count()`, or a truncated snippet — is deliberately left for the
/// [`Observer`] to compute so a disabled observer pays nothing. Emitters must
/// not pre-compute those values before handing an event to an observer.
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
    /// A contiguous day–month–year run was grouped into one atomic span.
    DateSequenceMatched { start: usize, end: usize },
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
