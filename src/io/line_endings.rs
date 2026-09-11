//! The line-ending policy: the style an output document is written with.
//!
//! [`detect_line_ending`] counts the document's line feed and carriage return
//! and line feed (CRLF) endings and selects the style holding the strict
//! majority, and [`serialize_lines`] re-terminates every reformatted line with
//! that style. Detection is per document, so no transform module needs to know
//! which terminator the source file uses.
//!
//! Everything here is a pure query over the text it is given, and nothing in
//! this module emits. The I/O boundary that formats a document reports the
//! decision afterwards: the rewrite helpers in `src/io/replace.rs` and the
//! executable's input/output boundaries.
//!
//! The rationale, the rejected alternatives and the known limitations are
//! recorded in `docs/adrs/0007-line-ending-detection.md`.

/// The line-ending style of a document.
///
/// Rewriting preserves the style that dominates the input so that a file
/// authored on Windows is not silently rewritten to line feeds, which would
/// otherwise show up as a whole-file diff that changes no Markdown content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    /// ```rust
    /// use mdtablefix::LineEnding;
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
}

/// Selects the line-ending style holding a strict majority of `text`'s line
/// endings.
///
/// CRLF pairs are counted first and then subtracted from the total line-feed
/// count to obtain the lone line feeds. Counting line feeds directly would
/// count every CRLF twice and leave CRLF unable to win.
/// [`LineEnding::Crlf`] is selected only when CRLF pairs strictly outnumber
/// lone line feeds.
///
/// Text with an exact tie and text with no line endings at all both select
/// [`LineEnding::Lf`], so the result is deterministic and never depends on
/// which style happens to appear first.
///
/// Only CRLF and LF are recognized. A lone carriage return is content rather
/// than a line ending, matching the `str::lines` split used to read the
/// document.
///
/// # Examples
///
/// ```rust
/// use mdtablefix::{LineEnding, detect_line_ending};
///
/// assert_eq!(detect_line_ending("alpha\nbeta\n"), LineEnding::Lf);
/// assert_eq!(detect_line_ending("alpha\r\nbeta\r\n"), LineEnding::Crlf);
/// assert_eq!(detect_line_ending("alpha\r\nbeta\n"), LineEnding::Lf);
/// assert_eq!(detect_line_ending("alpha"), LineEnding::Lf);
/// ```
#[must_use]
pub fn detect_line_ending(text: &str) -> LineEnding { count_line_endings(text).ending }

/// The line-ending counts of a document, and the style they select.
///
/// [`count_line_endings`] returns this so a caller can report or act on how
/// one-sided the majority vote was, rather than only on its outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineEndingCounts {
    /// The style the counts select.
    pub ending: LineEnding,
    /// The number of carriage return and line feed (CRLF) pairs.
    pub crlf_count: usize,
    /// The number of lone line feeds, which no carriage return precedes.
    pub lone_lf_count: usize,
}

/// Counts `text`'s line endings and selects the majority style.
///
/// This is [`detect_line_ending`] with the counts that decided it, so a caller
/// that reports or acts on the vote does not have to restate the counting
/// rule. CRLF pairs are counted first and subtracted from the total line-feed
/// count to obtain the lone line feeds.
///
/// # Examples
///
/// ```rust
/// use mdtablefix::{LineEnding, count_line_endings};
///
/// let counts = count_line_endings("alpha\r\nbeta\r\ngamma\n");
///
/// assert_eq!(counts.ending, LineEnding::Crlf);
/// assert_eq!(counts.crlf_count, 2);
/// assert_eq!(counts.lone_lf_count, 1);
/// ```
#[must_use]
pub fn count_line_endings(text: &str) -> LineEndingCounts {
    let crlf_count = text.matches("\r\n").count();
    let lone_lf_count = text.matches('\n').count() - crlf_count;
    let ending = if crlf_count > lone_lf_count {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    };
    LineEndingCounts {
        ending,
        crlf_count,
        lone_lf_count,
    }
}

/// Renders `lines` as one document whose lines end with `ending`.
///
/// An empty slice yields an empty string. Otherwise every line is followed by
/// `ending`, so a non-empty result always carries one trailing terminator.
///
/// # Examples
///
/// ```rust
/// use mdtablefix::{LineEnding, serialize_lines};
///
/// let lines = vec!["| A |".to_string(), "| 1 |".to_string()];
///
/// assert_eq!(serialize_lines(&lines, LineEnding::Lf), "| A |\n| 1 |\n");
/// assert_eq!(
///     serialize_lines(&lines, LineEnding::Crlf),
///     "| A |\r\n| 1 |\r\n"
/// );
/// assert_eq!(serialize_lines(&[], LineEnding::Crlf), "");
/// ```
#[must_use]
pub fn serialize_lines(lines: &[String], ending: LineEnding) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let terminator = ending.as_str();
    let capacity: usize = lines.iter().map(|line| line.len() + terminator.len()).sum();
    let mut output = String::with_capacity(capacity);
    for line in lines {
        output.push_str(line);
        output.push_str(terminator);
    }
    output
}
