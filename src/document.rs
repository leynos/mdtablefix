//! Document boundary: byte-order marks and line-ending styles.
//!
//! A document's byte-order mark and line-ending style are boundary concerns,
//! not content concerns. Both are split off before formatting and restored
//! afterwards, so every content transform sees the same lines regardless of
//! how the file was authored, and so a Windows-authored file is not silently
//! rewritten to line feeds.

/// The line-ending style used by a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// A single line feed, `\n`.
    Lf,
    /// A carriage return followed by a line feed, `\r\n`.
    Crlf,
}

impl LineEnding {
    /// Returns the characters written between lines.
    ///
    /// # Examples
    ///
    /// ```
    /// use mdtablefix::document::LineEnding;
    ///
    /// assert_eq!(LineEnding::Lf.as_str(), "\n");
    /// assert_eq!(LineEnding::Crlf.as_str(), "\r\n");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }

    /// Selects the style holding a strict majority of `content`'s line
    /// endings, defaulting to [`LineEnding::Lf`] on a tie or when there are
    /// none.
    ///
    /// Counts CRLF occurrences, then subtracts them from the total line-feed
    /// count to obtain lone line feeds. Counting line feeds without that
    /// subtraction double-counts every CRLF and makes CRLF unable to win.
    ///
    /// # Examples
    ///
    /// ```
    /// use mdtablefix::document::LineEnding;
    ///
    /// assert_eq!(LineEnding::detect("a\r\nb\r\n"), LineEnding::Crlf);
    /// assert_eq!(LineEnding::detect("a\r\nb\n"), LineEnding::Lf);
    /// assert_eq!(LineEnding::detect(""), LineEnding::Lf);
    /// ```
    #[must_use]
    pub fn detect(content: &str) -> Self {
        let carriage_returns = content.matches("\r\n").count();
        let lone_line_feeds = content.matches('\n').count() - carriage_returns;
        if carriage_returns > lone_line_feeds {
            Self::Crlf
        } else {
            Self::Lf
        }
    }
}

/// A parsed document: its lines, the line-ending style to restore, and
/// whether it began with a byte-order mark.
///
/// The byte-order mark is split off before formatting because leaving it
/// attached to the first line prevents every content transform from matching,
/// which would make `--check` report a ragged file as clean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    has_byte_order_mark: bool,
    lines: Vec<String>,
    line_ending: LineEnding,
}

impl SourceDocument {
    /// Splits `content` into lines, recording its byte-order mark and
    /// majority line-ending style.
    ///
    /// # Examples
    ///
    /// ```
    /// use mdtablefix::document::SourceDocument;
    ///
    /// let document = SourceDocument::parse("\u{FEFF}|A|B|\r\n|1|2|\r\n");
    /// assert_eq!(document.lines()[0], "|A|B|");
    /// assert_eq!(
    ///     document.render_lines(document.lines()),
    ///     "\u{FEFF}|A|B|\r\n|1|2|\r\n"
    /// );
    /// ```
    #[must_use]
    pub fn parse(content: &str) -> Self {
        let (has_byte_order_mark, body) = match content.strip_prefix('\u{FEFF}') {
            Some(body) => (true, body),
            None => (false, content),
        };
        Self {
            has_byte_order_mark,
            lines: body.lines().map(str::to_string).collect(),
            line_ending: LineEnding::detect(body),
        }
    }

    /// Borrows the parsed lines, with line endings and any byte-order mark
    /// removed.
    #[must_use]
    pub fn lines(&self) -> &[String] { &self.lines }

    /// Serializes `lines` using this document's byte-order mark and
    /// line-ending style.
    ///
    /// An empty slice yields an empty string. A non-empty slice is joined
    /// with the line ending and terminated with one further line ending.
    ///
    /// This is a method rather than a free function so a caller cannot pass a
    /// line-ending style belonging to a different document.
    ///
    /// # Examples
    ///
    /// ```
    /// use mdtablefix::document::SourceDocument;
    ///
    /// let document = SourceDocument::parse("a\r\nb\r\n");
    /// assert_eq!(document.render_lines(&["x".to_string()]), "x\r\n");
    /// assert_eq!(document.render_lines(&[]), "");
    /// ```
    #[must_use]
    pub fn render_lines(&self, lines: &[String]) -> String {
        if lines.is_empty() {
            return String::new();
        }
        let ending = self.line_ending.as_str();
        let body_len: usize = lines.iter().map(|line| line.len() + ending.len()).sum();
        let mark_len = if self.has_byte_order_mark {
            '\u{FEFF}'.len_utf8()
        } else {
            0
        };
        let mut rendered = String::with_capacity(body_len + mark_len);
        if self.has_byte_order_mark {
            rendered.push('\u{FEFF}');
        }
        for line in lines {
            rendered.push_str(line);
            rendered.push_str(ending);
        }
        rendered
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the document boundary.

    use rstest::rstest;

    use super::{LineEnding, SourceDocument};

    #[test]
    fn line_ending_renders_its_characters() {
        assert_eq!(LineEnding::Lf.as_str(), "\n");
        assert_eq!(LineEnding::Crlf.as_str(), "\r\n");
    }

    #[rstest]
    #[case::no_ending("", LineEnding::Lf)]
    #[case::line_feeds_only("alpha\nbeta\n", LineEnding::Lf)]
    #[case::carriage_returns_only("alpha\r\nbeta\r\n", LineEnding::Crlf)]
    #[case::tie("alpha\r\nbeta\n", LineEnding::Lf)]
    #[case::carriage_return_majority("a\r\nb\r\nc\n", LineEnding::Crlf)]
    #[case::line_feed_majority("a\nb\nc\r\n", LineEnding::Lf)]
    fn detect_selects_the_majority_style(#[case] content: &str, #[case] expected: LineEnding) {
        assert_eq!(LineEnding::detect(content), expected);
    }

    #[test]
    fn parse_splits_the_byte_order_mark_from_the_first_line() {
        let document = SourceDocument::parse("\u{FEFF}|A|B|\n|1|2|\n");
        assert_eq!(
            document.lines().to_vec(),
            vec!["|A|B|".to_string(), "|1|2|".to_string()]
        );
    }

    #[test]
    fn parse_records_the_dominant_line_ending() {
        let document = SourceDocument::parse("a\r\nb\r\nc\n");
        assert_eq!(document.render_lines(document.lines()), "a\r\nb\r\nc\r\n");
    }

    #[rstest]
    #[case::empty(vec![], "")]
    #[case::single(vec!["alpha".to_string()], "alpha\n")]
    #[case::pair(
        vec!["alpha".to_string(), "beta".to_string()],
        "alpha\nbeta\n"
    )]
    fn render_lines_joins_and_terminates(#[case] lines: Vec<String>, #[case] expected: &str) {
        let document = SourceDocument::parse("");
        assert_eq!(document.render_lines(&lines), expected);
    }

    #[test]
    fn render_lines_uses_the_document_ending() {
        let document = SourceDocument::parse("a\r\nb\r\n");
        assert_eq!(
            document.render_lines(&["x".to_string(), "y".to_string()]),
            "x\r\ny\r\n"
        );
    }

    #[test]
    fn render_lines_restores_the_byte_order_mark() {
        let document = SourceDocument::parse("\u{FEFF}alpha\n");
        assert_eq!(document.render_lines(&["x".to_string()]), "\u{FEFF}x\n");
    }

    #[test]
    fn render_lines_drops_the_byte_order_mark_when_nothing_is_rendered() {
        let document = SourceDocument::parse("\u{FEFF}alpha\n");
        assert_eq!(document.render_lines(&[]), "");
    }

    #[rstest]
    #[case::byte_order_mark_and_carriage_returns(
        "\u{FEFF}|A|B|\r\n|1|2|\r\n",
        "\u{FEFF}|A|B|\r\n|1|2|\r\n"
    )]
    #[case::line_feeds("alpha\nbeta\n", "alpha\nbeta\n")]
    #[case::missing_trailing_newline("alpha\nbeta", "alpha\nbeta\n")]
    #[case::empty("", "")]
    #[case::tie_prefers_line_feeds("alpha\r\nbeta\n", "alpha\nbeta\n")]
    #[case::lone_carriage_return("alpha\rbeta\n", "alpha\rbeta\n")]
    fn parse_then_render_round_trips(#[case] content: &str, #[case] expected: &str) {
        let document = SourceDocument::parse(content);
        assert_eq!(document.render_lines(document.lines()), expected);
    }
}
