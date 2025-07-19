//! High-level Markdown stream processing.

use crate::{
    ellipsis::replace_ellipsis,
    fences::{attach_orphan_specifiers, compress_fences},
    footnotes::convert_footnotes,
    html::convert_html_tables,
    table::reflow_table,
    wrap::{self, wrap_text},
};

/// Processing options controlling the behaviour of `process_stream_inner`.
#[derive(Clone, Copy)]
pub struct Options {
    /// Enable paragraph wrapping
    pub wrap: bool,
    /// Replace `...` with `…`
    pub ellipsis: bool,
    /// Normalise code block fences
    pub fences: bool,
}

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
        if wrap::is_fence(line) {
            if !buf.is_empty() {
                if in_table {
                    out.extend(reflow_table(&buf));
                } else {
                    out.extend(buf.clone());
                }
                buf.clear();
            }
            in_code = !in_code;
            out.push(line.to_string());
            continue;
        }

        if in_code {
            out.push(line.to_string());
            continue;
        }

        if line.trim_start().starts_with('|') {
            if !in_table {
                in_table = true;
            }
            buf.push(line.trim_end().to_string());
            continue;
        }

        if in_table && !line.trim().is_empty() {
            buf.push(line.trim_end().to_string());
            continue;
        }

        if !buf.is_empty() {
            if in_table {
                out.extend(reflow_table(&buf));
            } else {
                out.extend(buf.clone());
            }
            buf.clear();
            in_table = false;
        }

        out.push(line.to_string());
    }

    if !buf.is_empty() {
        if in_table {
            out.extend(reflow_table(&buf));
        } else {
            out.extend(buf);
        }
    }

    let mut out = if opts.wrap { wrap_text(&out, 80) } else { out };
    if opts.ellipsis {
        out = replace_ellipsis(&out);
    }
    if footnotes {
        out = convert_footnotes(&out);
    }
    out
}

#[must_use]
pub fn process_stream(lines: &[String]) -> Vec<String> {
    process_stream_inner(
        lines,
        Options {
            wrap: true,
            ellipsis: false,
            fences: false,
        },
    )
}

#[must_use]
pub fn process_stream_no_wrap(lines: &[String]) -> Vec<String> {
    process_stream_inner(
        lines,
        Options {
            wrap: false,
            ellipsis: false,
            fences: false,
        },
    )
}

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
}
