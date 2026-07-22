//! High-level Markdown stream processing.

mod buffer;

use buffer::ProcessBuffer;

use crate::{
    ellipsis::replace_ellipsis,
    fences::{attach_orphan_specifiers, compress_fences},
    footnotes::convert_footnotes,
    frontmatter::split_leading_yaml_frontmatter,
    html::convert_html_tables,
    wrap::{FenceTracker, wrap_text},
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

    let mut state = ProcessBuffer::new(opts.ellipsis);
    // Track fences so subsequent logic respects shared semantics.
    let mut fence_tracker = FenceTracker::default();

    for line in pre {
        if state.handle_fence_line(&line, &mut fence_tracker) {
            continue;
        }

        if fence_tracker.in_fence() {
            state.push_out(line);
            continue;
        }

        let Some(line) = state.handle_table_line(line) else {
            continue;
        };

        state.flush();
        state.push_out(line);
    }

    state.flush();

    let mut out = state.into_out();
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
