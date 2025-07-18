//! Library for fixing Markdown tables and wrapping text.
//!
//! Modules:
//! - `html` for converting HTML tables.
//! - `table` for Markdown table alignment.
//! - `wrap` for paragraph wrapping.
//! - `lists` for renumbering ordered lists.
//! - `breaks` for thematic break formatting.
//! - `ellipsis` for normalising textual ellipses.
//! - `process` for stream processing.
//! - `io` for file helpers.

pub mod breaks;
pub mod ellipsis;
mod html;
pub mod io;
pub mod lists;
pub mod process;
mod reflow;
pub mod table;
pub mod wrap;

#[doc(hidden)]
#[must_use]
pub fn html_table_to_markdown(lines: &[String]) -> Vec<String> {
    html::html_table_to_markdown(lines)
}

pub use breaks::{THEMATIC_BREAK_LEN, format_breaks};
pub use ellipsis::replace_ellipsis;
pub use html::convert_html_tables;
pub use io::{rewrite, rewrite_no_wrap};
pub use lists::renumber_lists;
pub use process::{process_stream, process_stream_no_wrap, process_stream_opts};
pub use table::{reflow_table, split_cells};
pub use wrap::{is_fence, wrap_text};
