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
#[derive(Debug, Clone, Copy)]
pub(crate) enum Event<'a> {
    /// A complete footnote reference was parsed.
    FootnoteReferenceParsed { token_length: usize },
    /// A Markdown link or image was parsed.
    LinkOrImageParsed { token_length: usize, is_image: bool },
    /// A footnote label could not be completed.
    FootnoteEndNotFound { start: usize, reason: &'static str },
    /// A footnote label span was recognised.
    FootnoteLabelRecognized {
        start: usize,
        end: usize,
        token_length: usize,
    },
    /// A footnote-reference predicate was evaluated.
    FootnoteRefChecked { token: &'a str, result: bool },
    /// A rendered fragment was classified.
    FragmentClassified {
        token: &'a str,
        truncated: bool,
        kind: FragmentKind,
    },
}

/// Receives domain-level classification events.
pub(crate) trait Observer {
    /// Records one event.
    fn observe(&mut self, event: Event<'_>);
}

/// Discards every event.
#[allow(
    dead_code,
    reason = "The domain boundary provides a no-op observer for direct callers."
)]
pub(crate) struct NoOpObserver;

impl Observer for NoOpObserver {
    fn observe(&mut self, _: Event<'_>) {}
}

impl fmt::Display for FragmentKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
