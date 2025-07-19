//! High-level Markdown stream processing.

use crate::{
    ellipsis::replace_ellipsis,
    fences::{attach_orphan_specifiers, compress_fences},
    footnotes::convert_footnotes,
    html::convert_html_tables,
    table::reflow_table,
    wrap::{self, wrap_text},
};

#[must_use]
pub fn process_stream_inner(
    lines: &[String],
    wrap: bool,
    ellipsis: bool,
    fences: bool,
    footnotes: bool,
) -> Vec<String> {
    let lines = if fences {
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

    let mut out = if wrap { wrap_text(&out, 80) } else { out };
    if ellipsis {
        out = replace_ellipsis(&out);
    }
    if footnotes {
        out = convert_footnotes(&out);
    }
    out
}

#[must_use]
pub fn process_stream(lines: &[String]) -> Vec<String> {
    process_stream_inner(lines, true, false, false)
}

#[must_use]
pub fn process_stream_no_wrap(lines: &[String]) -> Vec<String> {
    process_stream_inner(lines, false, false, false)
}

#[must_use]
pub fn process_stream_opts(
    lines: &[String],
    wrap: bool,
    ellipsis: bool,
    fences: bool,
    footnotes: bool,
) -> Vec<String> {
    process_stream_inner(lines, wrap, ellipsis, fences, footnotes)
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
