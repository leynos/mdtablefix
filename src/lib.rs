//! Library for normalizing Markdown tables and wrapping text.
//!
//! Modules:
//! - `html` for converting HTML tables.
//! - `table` for standardizing Markdown table alignment.
//! - `wrap` for paragraph wrapping.
//! - `lists` for renumbering ordered lists.
//! - `breaks` for thematizing horizontal rules.
//! - `ellipsis` for replacing textual ellipses.
//! - `fences` for issues with code block fences
//! - `footnotes` for converting bare footnote links.
//! - `headings` for standardizing Setext headings.
//! - `code_emphasis` for fixing emphasis adjoining inline code.
//! - `textproc` for token-based transformations.
//! - `process` for stream processing.
//! - `report` for line-delta reporting.
//! - `io` for file helpers, the document boundary, and line-ending preservation.

/// Create a lazily initialized regular expression.
///
/// The pattern is compiled on first access. If it is invalid, initialization
/// panics with the supplied message and the regex error. The expansion names
/// `regex::Regex`, so the calling crate must depend on `regex` directly.
///
/// This macro is the crate's single deliberate panic boundary for regular
/// expressions. Every call site passes a string literal, so an invalid pattern
/// is a programming error caught the first time the static is touched, not a
/// runtime condition a caller could handle. Routing the failure through an
/// explicit `match` states that intent at the one place it can occur, rather
/// than spreading a fallible initializer through every use of every static.
///
/// # Examples
///
/// ```
/// use std::sync::LazyLock;
///
/// static WORD: LazyLock<regex::Regex> =
///     mdtablefix::lazy_regex!(r"[[:alpha:]]+", "word pattern should compile");
///
/// assert!(WORD.is_match("table"));
/// ```
#[macro_export]
macro_rules! lazy_regex {
    ($re:expr, $msg:expr $(,)?) => {
        std::sync::LazyLock::new(|| match regex::Regex::new($re) {
            Ok(compiled) => compiled,
            Err(err) => panic!("{}: {err}", $msg),
        })
    };
}

pub mod breaks;
mod classify;
mod classify_kernel;
#[cfg(test)]
mod classify_kernel_tests;
pub mod code_emphasis;
pub mod ellipsis;
pub mod fences;
pub mod footnotes;
pub(crate) mod frontmatter;
pub mod headings;
mod html;
pub mod io;
mod list_interrupt;
pub mod lists;
pub mod process;
mod reflow;
pub mod report;
pub mod table;
pub mod textproc;
pub mod wrap;

/// Convert HTML tables in `lines` to Markdown tables.
///
/// Legacy entry point kept for compatibility; use [`convert_html_tables`] in new
/// code.
///
/// # Examples
///
/// ```
/// # #![expect(deprecated, reason = "this example exercises the legacy wrapper")]
/// use mdtablefix::html_table_to_markdown;
///
/// let html = vec![
///     "<table>".to_string(),
///     "<tr><th>A</th></tr>".to_string(),
///     "<tr><td>1</td></tr>".to_string(),
///     "</table>".to_string(),
/// ];
/// let markdown = html_table_to_markdown(&html);
/// assert!(markdown.iter().any(|line| line.contains('A')));
/// ```
///
/// # Migration example
///
/// ```
/// use mdtablefix::convert_html_tables;
///
/// let html = vec![
///     "<table>".to_string(),
///     "<tr><th>Header</th></tr>".to_string(),
///     "<tr><td>Cell</td></tr>".to_string(),
///     "</table>".to_string(),
/// ];
/// let markdown = convert_html_tables(&html);
///
/// assert_eq!(markdown.first().map(String::as_str), Some("| Header |"));
/// assert_eq!(markdown.last().map(String::as_str), Some("| Cell   |"));
/// assert!(markdown.iter().any(|line| line.starts_with("| ---")));
/// ```
#[deprecated(
    since = "0.1.0",
    note = "this function is legacy; use `convert_html_tables` instead"
)]
#[must_use]
pub fn html_table_to_markdown(lines: &[String]) -> Vec<String> {
    html::html_table_to_markdown(lines)
}

pub use breaks::{THEMATIC_BREAK_LEN, format_breaks};
pub use code_emphasis::fix_code_emphasis;
pub use ellipsis::replace_ellipsis;
pub use fences::{attach_orphan_specifiers, compress_fences};
pub use footnotes::convert_footnotes;
pub use headings::convert_setext_headings;
pub use html::convert_html_tables;
pub use io::{
    LineEnding,
    LineEndingCounts,
    count_line_endings,
    detect_line_ending,
    rewrite,
    rewrite_no_wrap,
    serialize_lines,
};
pub use lists::renumber_lists;
pub use process::{Options, process_stream, process_stream_no_wrap, process_stream_opts};
pub use table::{reflow_table, split_cells};
pub use wrap::{Token, is_fence, tokenize_markdown, wrap_text};
