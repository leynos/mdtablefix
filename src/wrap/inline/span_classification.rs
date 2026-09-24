//! Select atomic Markdown token spans before inline line fitting.
//!
//! This module classifies the first token and then extends it across attached
//! punctuation, references, links, and inline code without splitting UTF-8 text.

use tracing::trace;
use unicode_width::UnicodeWidthStr;

use super::{
    is_code_token,
    predicates::{
        ends_with_hyphen_prefix,
        is_opening_punct,
        is_trailing_punctuation_token,
        is_whitespace_token,
        looks_like_bracketed_reference,
        looks_like_footnote_ref,
        looks_like_link,
    },
    span_helpers::{
        SpanKind,
        absorb_token_and_trailing_punctuation,
        date_token_span,
        extend_punctuation,
        merge_code_span,
        should_couple_whitespace,
        try_couple_bracketed_reference,
        try_couple_footnote_reference,
        try_couple_inline_link_after_opener,
    },
    tracing_events::{emit_footnote_reference_coupling, emit_whitespace_footnote_coupling},
};

/// Classify an atomic token that must stay with its opening punctuation.
fn opening_coupling_kind(first: &str, next: &str) -> Option<SpanKind> {
    if is_code_token(next) {
        Some(SpanKind::Code)
    } else if looks_like_link(next) {
        Some(SpanKind::Link)
    } else if first == "[" && looks_like_bracketed_reference(next) {
        // Only `[` introduces a bare numeric reference.
        Some(SpanKind::BracketedRef)
    } else {
        None
    }
}

/// Finds the next logical token group starting at `start`.
///
/// `tokens` is the segmented inline token stream and `start` is the first
/// token in the next candidate group. The return value is `(end, width)`,
/// where `end` is the exclusive end index of the grouped inline code span,
/// link, or plain fragment, and `width` is its Unicode display width. An
/// empty or out-of-range stream returns a zero-width span at `start`.
pub(in crate::wrap) fn determine_token_span(tokens: &[String], start: usize) -> (usize, usize) {
    if tokens.get(start).is_none() {
        return (start, 0);
    }
    if let Some((end, width)) = date_token_span(tokens, start) {
        // Keep the diagnostic target stable when span selection lives here.
        trace!(
            target: "mdtablefix::wrap::inline",
            start,
            end, width, "determine_token_span grouped date sequence"
        );
        return (end, width);
    }

    let Some(mut span) = SpanCursor::new(tokens, start) else {
        return (start, 0);
    };
    while span.advance() {}
    (span.end, span.width)
}

/// Private cursor that owns one selection loop's borrowed token view and state.
struct SpanCursor<'a> {
    /// Immutable token stream being classified.
    tokens: &'a [String],
    /// First token, borrowed from `tokens` for all initial classification.
    first: &'a str,
    /// Index of `first` within `tokens`.
    start: usize,
    /// Exclusive end of the selected token span.
    end: usize,
    /// Unicode display width of the selected token span.
    width: usize,
    /// Markdown construct currently being extended.
    kind: SpanKind,
}

impl<'a> SpanCursor<'a> {
    /// Initialize the first atomic span and its attached punctuation.
    fn new(tokens: &'a [String], start: usize) -> Option<Self> {
        let first = tokens.get(start)?.as_str();
        let mut span = Self {
            tokens,
            first,
            start,
            end: start + 1,
            width: UnicodeWidthStr::width(first),
            kind: SpanKind::General,
        };
        span.couple_opening();
        span.couple_hyphen_prefix();
        span.classify_first();
        Some(span)
    }

    /// Keep opening punctuation with a following Markdown construct.
    fn couple_opening(&mut self) {
        // A lone opener at the end of a line would detach from its code or link.
        if !self.first.chars().all(is_opening_punct) {
            return;
        }
        let Some(next) = self.tokens.get(self.start + 1) else {
            return;
        };
        let Some(kind) = opening_coupling_kind(self.first, next) else {
            return;
        };
        self.kind = kind;
        self.end += 1;
        self.width += UnicodeWidthStr::width(next.as_str());
        self.end = extend_punctuation(self.tokens, self.end, &mut self.width);
    }

    /// Keep a hyphen prefix attached to the code span immediately after it.
    fn couple_hyphen_prefix(&mut self) {
        if self.kind != SpanKind::General {
            return;
        }
        if !ends_with_hyphen_prefix(self.first) {
            return;
        }
        let Some(next) = self
            .tokens
            .get(self.end)
            .filter(|token| is_code_token(token))
        else {
            return;
        };
        self.kind = SpanKind::Code;
        self.width += UnicodeWidthStr::width(next.as_str());
        self.end += 1;
        self.end = extend_punctuation(self.tokens, self.end, &mut self.width);
    }

    /// Classify code, link, or footnote tokens and attach trailing punctuation.
    fn classify_first(&mut self) {
        if self.first == "`" {
            self.kind = SpanKind::Code;
            self.end = merge_code_span(self.tokens, self.start, &mut self.width);
        } else if is_code_token(self.first) {
            self.kind = SpanKind::Code;
            self.end = extend_punctuation(self.tokens, self.end, &mut self.width);
        } else if looks_like_link(self.first) {
            self.kind = SpanKind::Link;
            self.end = extend_punctuation(self.tokens, self.end, &mut self.width);
        } else if looks_like_footnote_ref(self.first) {
            self.kind = SpanKind::FootnoteRef;
            self.end = extend_punctuation(self.tokens, self.end, &mut self.width);
        }
    }

    /// Consume the next token only when it belongs to the current atomic span.
    fn advance(&mut self) -> bool {
        let Some(token) = self.tokens.get(self.end) else {
            return false;
        };
        if is_whitespace_token(token) {
            return self.couple_whitespace();
        }
        if is_trailing_punctuation_token(token) {
            return self.couple_trailing_punctuation();
        }
        self.couple_attached_reference() || self.couple_adjacent_atom()
    }

    /// Keep whitespace only when the following token extends an atomic group.
    fn couple_whitespace(&mut self) -> bool {
        let next_token = self.tokens.get(self.end + 1);
        let following_token = self.tokens.get(self.end + 2);
        let should_couple = should_couple_whitespace(self.kind, next_token, following_token);
        emit_whitespace_footnote_coupling(self.kind, next_token, following_token, should_couple);
        if should_couple {
            let Some(token) = self.tokens.get(self.end) else {
                return false;
            };
            self.width += UnicodeWidthStr::width(token.as_str());
            self.end += 1;
        }
        should_couple
    }

    /// Attach punctuation after code, links, and footnote references.
    fn couple_trailing_punctuation(&mut self) -> bool {
        let should_couple = matches!(
            self.kind,
            SpanKind::Code | SpanKind::Link | SpanKind::FootnoteRef
        );
        if should_couple {
            let Some(token) = self.tokens.get(self.end) else {
                return false;
            };
            self.width += UnicodeWidthStr::width(token.as_str());
            self.end += 1;
        }
        should_couple
    }

    /// Attach a link, bare bracket reference, or footnote marker to its opener.
    fn couple_attached_reference(&mut self) -> bool {
        if let Some((kind, end)) =
            try_couple_inline_link_after_opener(self.tokens, self.end, &mut self.width)
        {
            self.kind = kind;
            self.end = end;
            return true;
        }

        // Bare bracket references need their opener to avoid stranding `[`.
        if let Some((kind, end)) =
            try_couple_bracketed_reference(self.tokens, self.end, &mut self.width)
        {
            self.kind = kind;
            self.end = end;
            return true;
        }

        let coupling =
            try_couple_footnote_reference(self.tokens, self.end, self.kind, &mut self.width);
        emit_footnote_reference_coupling(self.tokens, self.end, self.kind, coupling.is_some());
        if let Some((kind, end)) = coupling {
            self.kind = kind;
            self.end = end;
            return true;
        }
        false
    }

    /// Chain adjacent links or code spans without introducing a break point.
    fn couple_adjacent_atom(&mut self) -> bool {
        let Some(token) = self.tokens.get(self.end) else {
            return false;
        };
        let is_same_kind = match self.kind {
            SpanKind::Link => looks_like_link(token),
            SpanKind::Code => is_code_token(token),
            _ => false,
        };
        if is_same_kind {
            self.end =
                absorb_token_and_trailing_punctuation(self.tokens, self.end, &mut self.width);
        }
        is_same_kind
    }
}

#[cfg(test)]
mod tests {
    //! Boundary and Unicode checks for the span-selection cursor.

    use rstest::rstest;

    use super::determine_token_span;

    #[rstest]
    #[case::empty(&[], 0, (0, 0))]
    #[case::unicode(&["表"], 0, (1, 2))]
    #[case::at_end(&["表"], 1, (1, 0))]
    #[case::past_end(&["表"], usize::MAX, (usize::MAX, 0))]
    fn span_selection_handles_stream_boundaries_and_display_width(
        #[case] words: &[&str],
        #[case] start: usize,
        #[case] expected: (usize, usize),
    ) {
        let tokens = words.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(determine_token_span(&tokens, start), expected);
    }
}
