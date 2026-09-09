//! The document boundary: the byte-order mark and the line-ending style.
//!
//! A document's byte-order mark and line-ending style are boundary concerns,
//! not content concerns. Both are split off before formatting and restored
//! afterwards, so every content transform sees the same lines regardless of how
//! the file was authored, and so a Windows-authored file is not silently
//! rewritten to line feeds.
//!
//! [`SourceDocument`] binds both to the document they were read from, so a
//! caller cannot render one document's lines with another's style. The
//! line-ending policy behind it lives in [`super::line_endings`].
//!
//! Splitting the mark off matters for more than fidelity. Left attached to the
//! first line it defeats every content transform, which would make `--check`
//! report a ragged file as clean — a silent false negative rather than a
//! visible failure.

use super::line_endings::{LineEnding, LineEndingCounts, count_line_endings, serialize_lines};

/// The character a document may begin with to declare its byte order.
const BYTE_ORDER_MARK: char = '\u{FEFF}';

/// A parsed document: its body, the line-ending style to restore, and whether
/// it began with a byte-order mark.
///
/// Parsing is a pure query. Nothing here reads a file or emits an event; the
/// boundary that acts on the answer reports it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceDocument<'a> {
    has_byte_order_mark: bool,
    body: &'a str,
    counts: LineEndingCounts,
}

impl<'a> SourceDocument<'a> {
    /// Splits `content`'s byte-order mark from its body and counts the line
    /// endings of the body.
    ///
    /// The body is what content transforms see. Counting line endings over the
    /// body rather than over `content` is deliberate: the mark is not a line
    /// ending, and on a document that is only a mark it would otherwise be the
    /// sole reason a majority existed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mdtablefix::io::document::SourceDocument;
    ///
    /// let document = SourceDocument::parse("\u{FEFF}|A|B|\r\n|1|2|\r\n");
    ///
    /// assert_eq!(document.body(), "|A|B|\r\n|1|2|\r\n");
    /// assert_eq!(document.render(document.body().lines().map(str::to_string).collect::<Vec<_>>().as_slice()), "\u{FEFF}|A|B|\r\n|1|2|\r\n");
    /// ```
    #[must_use]
    pub fn parse(content: &'a str) -> Self {
        let (has_byte_order_mark, body) = match content.strip_prefix(BYTE_ORDER_MARK) {
            Some(body) => (true, body),
            None => (false, content),
        };
        Self {
            has_byte_order_mark,
            body,
            counts: count_line_endings(body),
        }
    }

    /// Borrows the body: `content` without a leading byte-order mark, and with
    /// its line endings intact.
    ///
    /// This is the text a content transform is given.
    #[must_use]
    pub const fn body(&self) -> &'a str { self.body }

    /// Returns the body's line-ending counts, and the style they select.
    ///
    /// The counts are what a boundary reports so a user can see how one-sided
    /// the majority was, rather than only which style won.
    #[must_use]
    pub const fn counts(&self) -> LineEndingCounts { self.counts }

    /// Returns the line-ending style this document's output is written with.
    ///
    /// This is [`Self::counts`]'s selected style, so a caller that only needs
    /// the terminator is not tempted to select one itself.
    #[must_use]
    pub const fn ending(&self) -> LineEnding { self.counts.ending }

    /// Renders `lines` with this document's byte-order mark and line-ending
    /// style.
    ///
    /// An empty slice renders the mark alone, or nothing when the document had
    /// none. That keeps a document which formats to no lines byte-identical to
    /// its input rather than truncating it, so `--check` stays a fixed point
    /// and `--in-place` does not destroy the mark.
    ///
    /// This is a method rather than a free function so a caller cannot pass a
    /// line-ending style belonging to a different document.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mdtablefix::io::document::SourceDocument;
    ///
    /// let document = SourceDocument::parse("a\r\nb\r\n");
    ///
    /// assert_eq!(document.ending().as_str(), "\r\n");
    /// assert_eq!(document.render(&["x".to_string()]), "x\r\n");
    /// assert_eq!(document.render(&[]), "");
    ///
    /// let marked = SourceDocument::parse("\u{FEFF}a\n");
    ///
    /// assert_eq!(marked.render(&["x".to_string()]), "\u{FEFF}x\n");
    /// assert_eq!(marked.render(&[]), "\u{FEFF}");
    /// ```
    #[must_use]
    pub fn render(&self, lines: &[String]) -> String {
        let body = serialize_lines(lines, self.ending());
        if !self.has_byte_order_mark {
            return body;
        }
        let mut rendered = String::with_capacity(body.len() + BYTE_ORDER_MARK.len_utf8());
        rendered.push(BYTE_ORDER_MARK);
        rendered.push_str(&body);
        rendered
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the document boundary.

    use rstest::rstest;

    use super::SourceDocument;

    fn lines(text: &str) -> Vec<String> { text.lines().map(str::to_string).collect() }

    #[test]
    fn parse_splits_the_byte_order_mark_from_the_body() {
        let document = SourceDocument::parse("\u{FEFF}|A|B|\n|1|2|\n");

        assert_eq!(document.body(), "|A|B|\n|1|2|\n");
        assert_eq!(document.counts().crlf_count, 0);
        assert_eq!(document.counts().lone_lf_count, 2);
    }

    #[rstest]
    #[case::empty("", "")]
    #[case::no_mark("alpha\n", "alpha\n")]
    #[case::mark_only("\u{FEFF}", "")]
    #[case::mark_and_body("\u{FEFF}alpha\n", "alpha\n")]
    fn parse_removes_only_the_leading_mark(#[case] content: &str, #[case] expected: &str) {
        assert_eq!(SourceDocument::parse(content).body(), expected);
    }

    #[test]
    fn parse_counts_the_body_not_the_mark() {
        let document = SourceDocument::parse("\u{FEFF}alpha\r\nbeta\r\n");

        assert_eq!(document.counts().crlf_count, 2);
        assert_eq!(document.counts().lone_lf_count, 0);
        assert_eq!(document.ending().as_str(), "\r\n");
    }

    #[rstest]
    #[case::empty(vec![], "")]
    #[case::single(lines("alpha\n"), "alpha\n")]
    #[case::pair(lines("alpha\nbeta\n"), "alpha\nbeta\n")]
    fn render_terminates_every_line(#[case] lines: Vec<String>, #[case] expected: &str) {
        assert_eq!(SourceDocument::parse("").render(&lines), expected);
    }

    #[test]
    fn render_uses_the_document_ending() {
        let document = SourceDocument::parse("a\r\nb\r\n");

        assert_eq!(
            document.render(&["x".to_string(), "y".to_string()]),
            "x\r\ny\r\n"
        );
    }

    #[test]
    fn render_restores_the_byte_order_mark() {
        let document = SourceDocument::parse("\u{FEFF}alpha\n");

        assert_eq!(document.render(&["x".to_string()]), "\u{FEFF}x\n");
    }

    #[test]
    fn render_keeps_a_mark_that_formats_to_no_lines() {
        let document = SourceDocument::parse("\u{FEFF}alpha\n");

        assert_eq!(document.render(&[]), "\u{FEFF}");
    }

    #[rstest]
    #[case::byte_order_mark_and_carriage_returns("\u{FEFF}|A|B|\r\n|1|2|\r\n")]
    #[case::line_feeds("alpha\nbeta\n")]
    #[case::empty("")]
    #[case::lone_carriage_return("alpha\rbeta\n")]
    fn parse_then_render_round_trips(#[case] content: &str) {
        let document = SourceDocument::parse(content);

        assert_eq!(
            document.render(&lines(document.body())),
            content,
            "a document whose endings are already uniform must be byte-identical"
        );
    }

    #[rstest]
    // A document with no strict majority is written with line feeds, so it is
    // deliberately not a round trip: `alpha\r\nbeta\n` is normalised.
    #[case::tie_prefers_line_feeds("alpha\r\nbeta\n", "alpha\nbeta\n")]
    #[case::missing_trailing_newline("alpha\nbeta", "alpha\nbeta\n")]
    fn parse_then_render_normalises(#[case] content: &str, #[case] expected: &str) {
        let document = SourceDocument::parse(content);

        assert_eq!(document.render(&lines(document.body())), expected);
    }
}
