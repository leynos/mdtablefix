//! Text wrapping utilities respecting inline code and prefixes.
//!
//! Unicode width handling follows the "Unicode Width Handling" section in
//! `docs/architecture.md` and uses the `unicode-width` crate for accurate
//! display calculations.

use regex::Regex;

static FENCE_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^\s*(```|~~~).*").unwrap());

static BULLET_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\s*(?:[-*+]|\d+[.)])\s+)(.*)").unwrap());

static FOOTNOTE_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\s*)(\[\^[^]]+\]:\s*)(.*)$").unwrap());

static BLOCKQUOTE_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\s*(?:>\s*)+)(.*)$").unwrap());

mod tokenizer;

pub(crate) use tokenizer::{Token, tokenize_markdown};

/// Determine if the current line should break at the last whitespace.
///
/// Returns `true` if `current_width` exceeds `width` and a whitespace split
/// position is available.
///
/// # Examples
///
/// ```ignore
/// use mdtablefix::wrap::should_break_line;
/// assert!(should_break_line(10, 12, Some(3)));
/// assert!(!should_break_line(10, 8, Some(3)));
/// ```
fn should_break_line(width: usize, current_width: usize, last_split: Option<usize>) -> bool {
    current_width > width && last_split.is_some()
}

fn wrap_preserving_code(text: &str, width: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    let mut last_split: Option<usize> = None;
    for token in tokenizer::tokenize_inline(text) {
        let token_width = UnicodeWidthStr::width(token.as_str());
        if current_width + token_width <= width {
            current.push_str(&token);
            current_width += token_width;
            if token.chars().all(char::is_whitespace) {
                last_split = Some(current.len());
            }
            continue;
        }

        if should_break_line(width, current_width + token_width, last_split) {
            let pos = last_split.unwrap();
            let line = current[..pos].to_string();
            let mut rest = current[pos..].trim_start().to_string();
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                lines.push(trimmed.to_string());
            }
            rest.push_str(&token);
            current = rest;
            current_width = UnicodeWidthStr::width(current.as_str());
            last_split = if token.chars().all(char::is_whitespace) {
                Some(current.len())
            } else {
                None
            };
            if current_width > width {
                lines.push(current.trim_end().to_string());
                current.clear();
                current_width = 0;
                last_split = None;
            }
            continue;
        }

        let trimmed = current.trim_end();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
        current.clear();
        current_width = 0;

        if !token.chars().all(char::is_whitespace) {
            current.push_str(&token);
            current_width = token_width;
        }
    }
    let trimmed = current.trim_end();
    if !trimmed.is_empty() {
        lines.push(trimmed.to_string());
    }
    lines
}

#[doc(hidden)]
pub fn is_fence(line: &str) -> bool { FENCE_RE.is_match(line) }

fn flush_paragraph(out: &mut Vec<String>, buf: &[(String, bool)], indent: &str, width: usize) {
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

fn append_wrapped_with_prefix(
    out: &mut Vec<String>,
    prefix: &str,
    text: &str,
    width: usize,
    repeat_prefix: bool,
) {
    use unicode_width::UnicodeWidthStr;

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

fn handle_prefix_line(
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

/// Wrap text lines to the given width.
///
/// # Panics
/// Panics if regex captures fail unexpectedly.
#[must_use]
pub fn wrap_text(lines: &[String], width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf: Vec<(String, bool)> = Vec::new();
    let mut indent = String::new();
    let mut in_code = false;

    for line in lines {
        if FENCE_RE.is_match(line) {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            in_code = !in_code;
            out.push(line.clone());
            continue;
        }

        if in_code {
            out.push(line.clone());
            continue;
        }

        if line.trim_start().starts_with('|') || crate::table::SEP_RE.is_match(line.trim()) {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            out.push(line.clone());
            continue;
        }

        if line.trim_start().starts_with('#') {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            out.push(line.clone());
            continue;
        }

        if line.trim().is_empty() {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            out.push(String::new());
            continue;
        }

        if let Some(cap) = BULLET_RE.captures(line) {
            let prefix = cap.get(1).unwrap().as_str();
            let rest = cap.get(2).unwrap().as_str();
            handle_prefix_line(&mut out, &mut buf, &mut indent, width, prefix, rest, false);
            continue;
        }

        if let Some(cap) = FOOTNOTE_RE.captures(line) {
            let indent_part = cap.get(1).unwrap().as_str();
            let label_part = cap.get(2).unwrap().as_str();
            let prefix = format!("{indent_part}{label_part}");
            let rest = cap.get(3).unwrap().as_str();
            handle_prefix_line(&mut out, &mut buf, &mut indent, width, &prefix, rest, false);
            continue;
        }

        if let Some(cap) = BLOCKQUOTE_RE.captures(line) {
            let prefix = cap.get(1).unwrap().as_str();
            let rest = cap.get(2).unwrap().as_str();
            handle_prefix_line(&mut out, &mut buf, &mut indent, width, prefix, rest, true);
            continue;
        }

        if buf.is_empty() {
            indent = line.chars().take_while(|c| c.is_whitespace()).collect();
        }
        let trimmed_end = line.trim_end();
        let text_without_html_breaks = trimmed_end
            .trim_end_matches("<br>")
            .trim_end_matches("<br/>")
            .trim_end_matches("<br />");

        let is_trailing_spaces = line.ends_with("  ");
        let is_html_br = trimmed_end != text_without_html_breaks;
        let backslash_count = line
            .trim_end()
            .chars()
            .rev()
            .take_while(|&c| c == '\\')
            .count();
        let is_backslash_escape = backslash_count % 2 == 1;

        let hard_break = is_trailing_spaces || is_html_br || is_backslash_escape;

        let text = text_without_html_breaks
            .trim_start()
            .trim_end_matches(' ')
            .to_string();

        buf.push((text, hard_break));
    }

    flush_paragraph(&mut out, &buf, &indent, width);
    out
}

#[cfg(test)]
mod tests;
