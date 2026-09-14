//! High-level Markdown stream processing.
mod buffer;
#[cfg(test)]
mod code_emphasis_tests;
mod pipeline;
mod table_line_protection;
use pipeline::{convert_footnote_references, normalize_fences, walk_fences_and_tables};
use table_line_protection::{protect_table_lines, restore_table_lines};

use crate::{
    ellipsis::replace_ellipsis,
    footnotes::convert_footnote_definitions,
    frontmatter::split_leading_yaml_frontmatter,
    html::convert_html_tables,
    lists::renumber_lists,
    wrap::wrap_text,
};
/// Column width used when wrapping text.
pub const WRAP_COLS: usize = 80;
/// Processing options controlling the behaviour of [`process_stream_inner`].
///
/// # Examples
///
/// ```
/// use mdtablefix::process::{Options, process_stream_opts};
///
/// let lines = vec!["example".to_string()];
/// let opts = Options {
///     wrap: false,
///     ellipsis: false,
///     fences: false,
///     footnotes: false,
///     renumber: false,
///     code_emphasis: false,
///     headings: false,
/// };
/// let out = process_stream_opts(&lines, opts);
/// assert_eq!(out, vec!["example"]);
/// ```
#[expect(
    clippy::struct_excessive_bools,
    reason = "Options map directly to CLI flags"
)]
#[derive(Clone, Copy, Default)]
pub struct Options {
    /// Enable paragraph wrapping.
    pub wrap: bool,
    /// Replace `...` with `…`.
    pub ellipsis: bool,
    /// Normalize code block fences.
    pub fences: bool,
    /// Convert bare numeric references into GitHub-flavoured footnote links (default: `false`).
    pub footnotes: bool,
    /// Renumber ordered list items.
    pub renumber: bool,
    /// Fix emphasis markers adjacent to inline code.
    pub code_emphasis: bool,
    /// Convert Setext-style headings into ATX (`#`) headings.
    pub headings: bool,
}

/// Processes a stream of Markdown lines using the provided [`Options`].
///
/// The function normalizes code fences, converts HTML tables, detects
/// Markdown tables and optionally wraps paragraphs. The exact behaviour is
/// controlled by `opts`.
///
/// # Examples
///
/// ```
/// use mdtablefix::process::{Options, process_stream_inner};
///
/// let lines = vec![
///     "| a | b |".to_string(),
///     "|---|---|".to_string(),
///     "| 1 | 2 |".to_string(),
/// ];
/// let out = process_stream_inner(
///     &lines,
///     Options {
///         wrap: false,
///         ellipsis: false,
///         fences: false,
///         footnotes: false,
///         renumber: false,
///         code_emphasis: false,
///         headings: false,
///     },
/// );
/// assert_eq!(
///     out,
///     vec![
///         "| a   | b   |".to_string(),
///         "| --- | --- |".to_string(),
///         "| 1   | 2   |".to_string(),
///     ]
/// );
/// ```
#[must_use]
pub fn process_stream_inner(lines: &[String], opts: Options) -> Vec<String> {
    let pre = convert_html_tables(&normalize_fences(lines, opts));
    let pre = convert_footnote_references(pre, opts);

    let (mut out, table_lines) = walk_fences_and_tables(pre, opts);

    if opts.headings {
        out = crate::headings::convert_setext_headings(&out);
    }
    if opts.code_emphasis {
        let (protected_lines, protected) = protect_table_lines(out, &table_lines);
        out = crate::code_emphasis::fix_code_emphasis(&protected_lines);
        out = restore_table_lines(out, &protected);
    }
    if opts.renumber {
        out = renumber_lists(&out);
    }

    // Layout is the final content-changing step for the blocks it owns. Each
    // normalizer above runs first so wrapping measures its final text.
    if opts.ellipsis {
        out = replace_ellipsis(&out);
    }

    if opts.wrap {
        out = wrap_text(&out, WRAP_COLS);
    }

    // The definition fold appends lines to the block structure the layout has
    // settled, so it runs last of all.
    if opts.footnotes {
        out = convert_footnote_definitions(&out);
    }

    out
}

/// Processes a Markdown stream with paragraph wrapping enabled.
///
/// This is the primary convenience function used by the command-line
/// interface. Paragraphs are wrapped and tables are reflowed.
///
/// # Examples
///
/// ```
/// use mdtablefix::process::process_stream;
///
/// let lines = vec![
///     "| a | b |".to_string(),
///     "|---|---|".to_string(),
///     "| 1 | 2 |".to_string(),
/// ];
/// let out = process_stream(&lines);
/// assert_eq!(
///     out,
///     vec![
///         "| a   | b   |".to_string(),
///         "| --- | --- |".to_string(),
///         "| 1   | 2   |".to_string(),
///     ]
/// );
/// ```
#[must_use]
pub fn process_stream(lines: &[String]) -> Vec<String> {
    process_stream_opts(
        lines,
        Options {
            wrap: true,
            ..Default::default()
        },
    )
}

/// Processes Markdown without wrapping paragraphs.
///
/// Useful when only table reflow and code fence normalization are required.
///
/// # Examples
///
/// ```
/// use mdtablefix::process::process_stream_no_wrap;
/// let lines = vec![
///     "| a | b |".to_string(),
///     "|---|---|".to_string(),
///     "| 1 | 2 |".to_string(),
/// ];
/// let out = process_stream_no_wrap(&lines);
/// assert_eq!(
///     out,
///     vec![
///         "| a   | b   |".to_string(),
///         "| --- | --- |".to_string(),
///         "| 1   | 2   |".to_string(),
///     ]
/// );
/// ```
#[must_use]
pub fn process_stream_no_wrap(lines: &[String]) -> Vec<String> {
    process_stream_opts(lines, Options::default())
}

/// Runs [`process_stream_inner`] with custom [`Options`].
///
/// This is exposed for advanced use cases where callers want precise
/// control over the processing pipeline. Set `footnotes: true` in `opts`
/// to convert bare numeric references into GitHub-flavoured footnote
/// links. The flag defaults to `false`.
///
/// # Examples
///
/// ```
/// use mdtablefix::process::{Options, process_stream_opts};
/// let lines = vec!["text".to_string()];
/// let opts = Options {
///     wrap: false,
///     ellipsis: false,
///     fences: false,
///     footnotes: false,
///     renumber: false,
///     code_emphasis: false,
///     headings: false,
/// };
/// let out = process_stream_opts(&lines, opts);
/// assert_eq!(out, vec!["text"]);
/// ```
#[must_use]
pub fn process_stream_opts(lines: &[String], opts: Options) -> Vec<String> {
    process_with_frontmatter(lines, |body| process_stream_inner(body, opts))
}

/// Processes a Markdown body while preserving leading YAML frontmatter verbatim.
///
/// This is the canonical frontmatter split/rejoin boundary. `body_fn` receives
/// only the post-frontmatter body slice; the leading frontmatter prefix is never
/// passed to it and is prepended verbatim to the closure's output.
///
/// # Examples
///
/// ```
/// use mdtablefix::process::process_with_frontmatter;
///
/// let lines = vec![
///     "---".to_string(),
///     "title: Example".to_string(),
///     "---".to_string(),
///     "markdown body".to_string(),
/// ];
/// let mut received = Vec::new();
/// let output = process_with_frontmatter(&lines, |body| {
///     received = body.to_vec();
///     body.iter().map(|line| line.to_uppercase()).collect()
/// });
///
/// assert_eq!(received, vec!["markdown body"]);
/// assert_eq!(
///     output,
///     vec!["---", "title: Example", "---", "MARKDOWN BODY"]
/// );
/// ```
#[must_use]
pub fn process_with_frontmatter<F>(lines: &[String], body_fn: F) -> Vec<String>
where
    F: FnOnce(&[String]) -> Vec<String>,
{
    let (frontmatter_prefix, body) = split_leading_yaml_frontmatter(lines);
    let mut result = frontmatter_prefix.to_vec();
    result.extend(body_fn(body));
    result
}

#[cfg(test)]
mod tests;
