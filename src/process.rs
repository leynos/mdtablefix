//! High-level Markdown stream processing.

use crate::{
    ellipsis::replace_ellipsis,
    fences::{attach_orphan_specifiers, compress_fences},
    footnotes::convert_footnotes,
    html::convert_html_tables,
    table::reflow_table,
    wrap::{FenceTracker, classify_block, wrap_text},
};

// Length of the YAML frontmatter header if present.
fn frontmatter_len(lines: &[String]) -> usize {
    if lines.first().is_some_and(|line| line.trim() == "---") {
        for (idx, line) in lines.iter().enumerate().skip(1) {
            if line.trim() == "---" {
                return idx + 1;
            }
        }
    }
    0
}

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
    /// Normalise code block fences.
    pub fences: bool,
    /// Convert bare numeric references into GitHub-flavoured footnote links (default: `false`).
    pub footnotes: bool,
    /// Fix emphasis markers adjacent to inline code.
    pub code_emphasis: bool,
    /// Convert Setext-style headings into ATX (`#`) headings.
    pub headings: bool,
}

/// Flushes buffered lines to `out`, formatting as a table when required.
fn flush_buffer(buf: &mut Vec<String>, in_table: &mut bool, out: &mut Vec<String>) {
    if buf.is_empty() {
        return;
    }
    if *in_table {
        out.extend(reflow_table(buf));
        buf.clear();
    } else {
        out.extend(std::mem::take(buf));
    }
    *in_table = false;
}

/// Detects fence lines and toggles code mode, flushing buffered content.
fn handle_fence_line(
    line: &str,
    buf: &mut Vec<String>,
    in_table: &mut bool,
    out: &mut Vec<String>,
    fences: &mut FenceTracker,
) -> bool {
    if !fences.observe(line) {
        return false;
    }

    flush_buffer(buf, in_table, out);
    out.push(line.to_string());
    true
}

/// Buffers table lines, returning `true` when a line was consumed.
fn handle_table_line(
    line: &str,
    buf: &mut Vec<String>,
    in_table: &mut bool,
    out: &mut Vec<String>,
) -> bool {
    let trimmed = line.trim_start();

    if trimmed.starts_with('|') {
        *in_table = true;
        buf.push(line.to_string());
        return true;
    }
    if line.trim().is_empty() {
        if *in_table {
            flush_buffer(buf, in_table, out);
        }
        return false;
    }
    if *in_table && (line.contains('|') || crate::table::SEP_RE.is_match(line.trim())) {
        buf.push(line.to_string());
        return true;
    }
    if *in_table {
        if classify_block(line).is_some() {
            // Flush when a new Markdown block (heading, list, quote, footnote, directive,
            // or digit-prefixed text) begins so wrapping and table detection stay aligned.
            flush_buffer(buf, in_table, out);
            return false;
        }
        // Plain paragraphs also end the table so the caller can reprocess them for wrapping.
        flush_buffer(buf, in_table, out);
        return false;
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
///         code_emphasis: false,
///         headings: false,
///     },
/// );
/// assert!(out.iter().any(|l| l.contains("| a | b |")));
/// ```
#[must_use]
pub fn process_stream_inner(lines: &[String], opts: Options) -> Vec<String> {
    let fm_len = frontmatter_len(lines);
    let (frontmatter, body) = lines.split_at(fm_len);
    let frontmatter = frontmatter.to_vec();

    let body = if opts.fences {
        let tmp = compress_fences(body);
        attach_orphan_specifiers(&tmp)
    } else {
        body.to_vec()
    };

    let pre = convert_html_tables(&body);

    let mut out = Vec::new();
    let mut buf = Vec::new();
    // Track fences so subsequent logic respects shared semantics.
    let mut fence_tracker = FenceTracker::default();
    let mut in_table = false;

    for line in &pre {
        if handle_fence_line(line, &mut buf, &mut in_table, &mut out, &mut fence_tracker) {
            continue;
        }

        if fence_tracker.in_fence() {
            out.push(line.to_string());
            continue;
        }

        if handle_table_line(line, &mut buf, &mut in_table, &mut out) {
            continue;
        }

        flush_buffer(&mut buf, &mut in_table, &mut out);
        out.push(line.to_string());
    }

    flush_buffer(&mut buf, &mut in_table, &mut out);

    if opts.headings {
        out = crate::headings::convert_setext_headings(&out);
    }
    if opts.code_emphasis {
        out = crate::code_emphasis::fix_code_emphasis(&out);
    }

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

    if fm_len > 0 {
        let mut combined = frontmatter;
        combined.extend(out);
        combined
    } else {
        out
    }
}

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
#[must_use]
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
///     code_emphasis: false,
///     headings: false,
/// };
/// let out = process_stream_opts(&lines, opts);
/// assert_eq!(out, vec!["text"]);
/// ```
#[must_use]
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

    #[test]
    fn integrates_code_emphasis_flag() {
        let input = vec!["`X`** Y (in **`Z`**)**".to_string()];
        let out = process_stream_inner(
            &input,
            Options {
                code_emphasis: true,
                ..Default::default()
            },
        );
        assert_eq!(out, vec!["**`X` Y (in `Z`)**"]);
    }

    #[test]
    fn converts_headings_when_enabled() {
        let input = vec![
            "Heading".to_string(),
            "====".to_string(),
            "Paragraph".to_string(),
        ];
        let disabled = process_stream_inner(
            &input,
            Options {
                headings: false,
                ..Default::default()
            },
        );
        assert_eq!(disabled, input);

        let enabled = process_stream_inner(
            &input,
            Options {
                headings: true,
                ..Default::default()
            },
        );
        assert_eq!(
            enabled,
            vec!["# Heading".to_string(), "Paragraph".to_string()]
        );
    }

    #[test]
    fn skips_frontmatter_processing() {
        let input = vec![
            "---".to_string(),
            "title: Example".to_string(),
            "---".to_string(),
            "| a | b |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        let output = process_stream(&input);

        assert_eq!(output[0..3], input[0..3]);
        assert!(output.iter().any(|line| line.contains("| a | b |")));
    }
}
