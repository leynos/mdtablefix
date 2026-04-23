//! High-level Markdown stream processing.

use crate::{
    ellipsis::replace_ellipsis,
    fences::{attach_orphan_specifiers, compress_fences},
    footnotes::convert_footnotes,
    frontmatter::split_leading_yaml_frontmatter,
    html::convert_html_tables,
    table::reflow_table,
    wrap::{FenceTracker, classify_block, wrap_text},
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
struct ProcessBuffer {
    out: Vec<String>,
    buf: Vec<String>,
    in_table: bool,
    ellipsis: bool,
}

impl ProcessBuffer {
    fn flush(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        let buffered = std::mem::take(&mut self.buf);
        if self.in_table {
            let table_lines = if self.ellipsis {
                replace_ellipsis(&buffered)
            } else {
                buffered
            };
            self.out.extend(reflow_table(&table_lines));
        } else {
            self.out.extend(buffered);
        }
        self.in_table = false;
    }

    fn push_verbatim(&mut self, line: &str) {
        self.flush();
        self.out.push(line.to_string());
    }

    fn handle_fence_line(&mut self, line: &str, fences: &mut FenceTracker) -> bool {
        if !fences.observe(line) {
            return false;
        }

        self.push_verbatim(line);
        true
    }

    fn handle_table_line(&mut self, line: &str) -> bool {
        if line.trim_start().starts_with('|') {
            self.in_table = true;
            self.buf.push(line.to_string());
            return true;
        }
        if line.trim().is_empty() {
            if self.in_table {
                self.flush();
            }
            return false;
        }
        if self.in_table && (line.contains('|') || crate::table::SEP_RE.is_match(line.trim())) {
            self.buf.push(line.to_string());
            return true;
        }
        if self.in_table {
            if classify_block(line).is_some() {
                // Flush when a new Markdown block begins so wrapping and table
                // detection stay aligned.
                self.flush();
                return false;
            }
            self.flush();
        }
        false
    }
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
    let lines = if opts.fences {
        let tmp = compress_fences(lines);
        attach_orphan_specifiers(&tmp)
    } else {
        lines.to_vec()
    };

    let pre = convert_html_tables(&lines);

    let mut state = ProcessBuffer {
        out: Vec::new(),
        buf: Vec::new(),
        in_table: false,
        ellipsis: opts.ellipsis,
    };
    // Track fences so subsequent logic respects shared semantics.
    let mut fence_tracker = FenceTracker::default();

    for line in &pre {
        if state.handle_fence_line(line, &mut fence_tracker) {
            continue;
        }

        if fence_tracker.in_fence() {
            state.out.push(line.clone());
            continue;
        }

        if state.handle_table_line(line) {
            continue;
        }

        state.flush();
        state.out.push(line.clone());
    }

    state.flush();

    let mut out = state.out;
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

    out
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
    process_with_frontmatter(
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
    process_with_frontmatter(lines, Options::default())
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
    process_with_frontmatter(lines, opts)
}

/// Helper to split frontmatter, process body, and rejoin.
fn process_with_frontmatter(lines: &[String], opts: Options) -> Vec<String> {
    let (frontmatter_prefix, body) = split_leading_yaml_frontmatter(lines);
    let out = process_stream_inner(body, opts);
    let mut result = frontmatter_prefix.to_vec();
    result.extend(out);
    result
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
        assert!(output.iter().any(|l| l.contains("| A   | B   |")));
        assert!(output.iter().any(|l| l.contains("| X   | Y   |")));
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
    fn process_stream_inner_applies_table_ellipsis_before_reflow() {
        let input = vec![
            "| example | value |".to_string(),
            "| ------- | ----- |".to_string(),
            "| ... | tail |".to_string(),
        ];

        let with_ellipsis = process_stream_inner(
            &input,
            Options {
                ellipsis: true,
                ..Default::default()
            },
        );
        let without_ellipsis = process_stream_inner(&input, Options::default());

        assert!(with_ellipsis.iter().any(|line| line.contains('…')));
        assert!(!with_ellipsis.iter().any(|line| line.contains("...")));
        assert!(without_ellipsis.iter().any(|line| line.contains("...")));
        assert!(!without_ellipsis.iter().any(|line| line.contains('…')));
    }
}
