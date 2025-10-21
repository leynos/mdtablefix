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
//! - `io` for file helpers.

#[macro_export]
macro_rules! lazy_regex {
    ($re:expr, $msg:expr $(,)?) => {
        std::sync::LazyLock::new(|| regex::Regex::new($re).expect($msg))
    };
}

pub mod breaks;
pub mod code_emphasis;
pub mod ellipsis;
pub mod fences;
pub mod footnotes;
pub mod headings;
mod html;
pub mod io;
pub mod lists;
pub mod process;
mod reflow;
pub mod table;
pub mod textproc;
pub mod wrap;

#[deprecated(note = "this function is legacy; use `convert_html_tables` instead")]
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
pub use io::{rewrite, rewrite_no_wrap};
pub use lists::renumber_lists;
pub use process::{Options, process_stream, process_stream_no_wrap, process_stream_opts};
pub use table::{reflow_table, split_cells};
pub use wrap::{Token, is_fence, tokenize_markdown, wrap_text};
