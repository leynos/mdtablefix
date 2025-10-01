//! Paragraph wrapping utilities shared by `wrap_text`.
//!
//! These helpers keep paragraph logic focused on buffer management while
//! deferring inline wrapping to `inline::wrap_preserving_code`.

use unicode_width::UnicodeWidthStr;

use super::inline::wrap_preserving_code;

fn append_wrapped_with_prefix(
    out: &mut Vec<String>,
    prefix: &str,
    text: &str,
    width: usize,
    repeat_prefix: bool,
) {
    let prefix_width = UnicodeWidthStr::width(prefix);
    let available = width.saturating_sub(prefix_width).max(1);
    let indent_str: String = prefix.chars().take_while(|c| c.is_whitespace()).collect();
    let indent_width = UnicodeWidthStr::width(indent_str.as_str());
    let wrapped_indent = if repeat_prefix {
        prefix.to_string()
    } else {
        format!("{}{}", indent_str, " ".repeat(prefix_width - indent_width))
    };

    let lines = wrap_preserving_code(text, available);
    if lines.is_empty() {
        out.push(prefix.to_string());
        return;
    }

    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            out.push(format!("{prefix}{line}"));
        } else {
            out.push(format!("{wrapped_indent}{line}"));
        }
    }
}

pub(super) fn flush_paragraph(
    out: &mut Vec<String>,
    buf: &[(String, bool)],
    indent: &str,
    width: usize,
) {
    if buf.is_empty() {
        return;
    }
    let mut segment = String::new();
    for (text, hard_break) in buf {
        if !segment.is_empty() {
            segment.push(' ');
        }
        segment.push_str(text);
        if *hard_break {
            for line in wrap_preserving_code(&segment, width - indent.len()) {
                out.push(format!("{indent}{line}"));
            }
            segment.clear();
        }
    }
    if !segment.is_empty() {
        for line in wrap_preserving_code(&segment, width - indent.len()) {
            out.push(format!("{indent}{line}"));
        }
    }
}

pub(super) fn handle_prefix_line(
    out: &mut Vec<String>,
    buf: &mut Vec<(String, bool)>,
    indent: &mut String,
    width: usize,
    prefix: &str,
    rest: &str,
    repeat_prefix: bool,
) {
    flush_paragraph(out, buf, indent, width);
    buf.clear();
    indent.clear();
    append_wrapped_with_prefix(out, prefix, rest, width, repeat_prefix);
}
