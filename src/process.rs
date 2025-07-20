//! High-level Markdown stream processing.

use crate::{
    ellipsis::replace_ellipsis,
    fences::{attach_orphan_specifiers, compress_fences},
    footnotes::convert_footnotes,
    html::convert_html_tables,
    table::reflow_table,
    wrap::{self, wrap_text},
};

/// Column width used when wrapping text.
pub(crate) const WRAP_COLS: usize = 80;

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
    /// Normalise code block fences.
    pub fences: bool,
    /// Convert bare numeric references to footnotes.
    pub footnotes: bool,
}

/// Flushes buffered lines to `out`, formatting as a table when required.
///
/// ```no_run
/// # use mdtablefix::table::reflow_table;
/// # fn flush_buffer(buf: &mut Vec<String>, in_table: &mut bool, out: &mut Vec<String>) {
/// #     if buf.is_empty() { return; }
/// #     if *in_table { out.extend(reflow_table(buf)); buf.clear(); } else { out.extend(buf.drain(..)); }
/// #     *in_table = false;
/// # }
/// let mut buf = vec!["| a | b |".to_string()];
/// let mut out = Vec::new();
/// let mut in_table = true;
/// flush_buffer(&mut buf, &mut in_table, &mut out);
/// assert!(buf.is_empty());
/// ```
fn flush_buffer(buf: &mut Vec<String>, in_table: &mut bool, out: &mut Vec<String>) {
    if buf.is_empty() {
        return;
    }
    if *in_table {
        out.extend(reflow_table(buf));
        buf.clear();
    } else {
        out.append(buf);
    }
    *in_table = false;
}

/// Detects fence lines and toggles code mode, flushing buffered content.
///
/// ```no_run
/// # use mdtablefix::process::{handle_fence_line, flush_buffer};
/// # use mdtablefix::table::reflow_table;
/// # fn flush_buffer(_: &mut Vec<String>, _: &mut bool, _: &mut Vec<String>) {}
/// let mut out = Vec::new();
/// let mut buf = Vec::new();
/// let mut in_code = false;
/// let mut in_table = false;
/// handle_fence_line("```", &mut buf, &mut in_code, &mut in_table, &mut out);
/// assert!(in_code);
/// ```
fn handle_fence_line(
    line: &str,
    buf: &mut Vec<String>,
    in_code: &mut bool,
    in_table: &mut bool,
    out: &mut Vec<String>,
) -> bool {
    if wrap::is_fence(line) {
        flush_buffer(buf, in_table, out);
        *in_code = !*in_code;
        out.push(line.to_string());
        return true;
    }
    false
}

/// Buffers table lines, returning `true` when a line was consumed.
///
/// ```no_run
/// # use mdtablefix::process::handle_table_line;
/// let mut buf = Vec::new();
/// let mut in_table = false;
/// assert!(handle_table_line("| a | b |", &mut buf, &mut in_table));
/// assert!(in_table);
/// ```
fn handle_table_line(line: &str, buf: &mut Vec<String>, in_table: &mut bool) -> bool {
    if line.trim_start().starts_with('|') {
        *in_table = true;
        buf.push(line.trim_end().to_string());
        return true;
    }
    if *in_table && !line.trim().is_empty() {
        buf.push(line.trim_end().to_string());
        return true;
    }
    false
}

/// Processes a stream of Markdown lines using the provided [`Options`].
///
/// The function normalises code fences, converts HTML tables, detects
/// Markdown tables and optionally wraps paragraphs. The exact behaviour is
/// controlled by `opts`.
///
/// # Examples
///
/// ```
/// use mdtablefix::process::{Options, process_stream_inner};
///
/// let lines = vec!["| a | b |".to_string(), "|---|---|".to_string()];
/// let out = process_stream_inner(
///     &lines,
///     Options {
///         wrap: false,
///         ellipsis: false,
///         fences: false,
///         footnotes: false,
///     },
/// );
/// assert!(out.iter().any(|l| l.contains("| a | b |")));
/// ```
#[must_use]
pub fn process_stream_inner(lines: &[String], opts: Options) -> Vec<String> {
    let lines = if opts.fences {
        let tmp = compress_fences(lines);
        attach_orphan_specifiers(&tmp)
    } else {
        lines.to_vec()
    };

    let pre = convert_html_tables(&lines);

    let mut out = Vec::new();
    let mut buf = Vec::new();
    let mut in_code = false;
    let mut in_table = false;

    for line in &pre {
        if handle_fence_line(line, &mut buf, &mut in_code, &mut in_table, &mut out) {
            continue;
        }

        if in_code {
            out.push(line.to_string());
            continue;
        }

        if handle_table_line(line, &mut buf, &mut in_table) {
            continue;
        }

        flush_buffer(&mut buf, &mut in_table, &mut out);
        out.push(line.to_string());
    }

    flush_buffer(&mut buf, &mut in_table, &mut out);

    let mut out = if opts.wrap {
        wrap_text(&out, WRAP_COLS)
    } else {
        out
    };
    if opts.ellipsis {
        out = replace_ellipsis(&out);
    }
    if opts.footnotes {
        out = convert_footnotes(&out);
    }
    out
}

#[must_use]
/// Processes a Markdown stream with all default options enabled.
///
/// This is the primary convenience function used by the command-line
/// interface. Paragraphs are wrapped and tables are reflowed.
///
/// # Examples
///
/// ```
/// use mdtablefix::process::process_stream;
///
/// let lines = vec!["| a | b |".to_string(), "|---|---|".to_string()];
/// let out = process_stream(&lines);
/// assert!(out.iter().any(|l| l.contains("| a | b |")));
/// ```
pub fn process_stream(lines: &[String]) -> Vec<String> {
    process_stream_inner(
        lines,
        Options {
            wrap: true,
            ..Default::default()
        },
    )
}

/// Processes Markdown without wrapping paragraphs.
///
/// Useful when only table reflow and code fence normalisation are required.
///
/// # Examples
///
/// ```
/// use mdtablefix::process::process_stream_no_wrap;
/// let lines = vec!["| a | b |".to_string(), "|---|---|".to_string()];
/// let out = process_stream_no_wrap(&lines);
/// assert!(out.iter().any(|l| l.contains("| a | b |")));
/// ```
#[must_use]
#[inline]
pub fn process_stream_no_wrap(lines: &[String]) -> Vec<String> {
    process_stream_inner(lines, Options::default())
}

#[must_use]
/// Runs [`process_stream_inner`] with custom [`Options`].
///
/// This is exposed for advanced use cases where callers want precise
/// control over the processing pipeline.
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
/// };
/// let out = process_stream_opts(&lines, opts);
/// assert_eq!(out, vec!["text"]);
/// ```
pub fn process_stream_opts(lines: &[String], opts: Options) -> Vec<String> {
    process_stream_inner(lines, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processes_html_and_tables() {
        let input = vec![
            "<table><tr><td>A</td><td>B</td></tr></table>".to_string(),
            "| X | Y |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        let output = process_stream(&input);
        assert!(output.iter().any(|l| l.contains("| A | B |")));
        assert!(output.iter().any(|l| l.contains("| X | Y |")));
    }

    #[test]
    fn no_wrap_option() {
        let input = vec!["| a | b |".to_string(), "| 1 | 2 |".to_string()];
        let out = process_stream_no_wrap(&input);
        assert_eq!(out, vec!["| a | b |", "| 1 | 2 |"]);
    }
}
