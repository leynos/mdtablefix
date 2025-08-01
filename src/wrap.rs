//! Utilities for wrapping Markdown lines.
//!
//! These helpers reflow paragraphs and list items while preserving inline code
//! spans, fenced code blocks, and other prefixes. Width calculations rely on
//! `UnicodeWidthStr::width` from the `unicode-width` crate as described in
//! `docs/architecture.md#unicode-width-handling`.
//!
//! The [`Token`] enum and [`tokenize_markdown`] function are public so callers
//! can perform custom token-based processing.

use regex::{Captures, Regex};

mod tokenize;
/// Token emitted by [`tokenize::segment_inline`] and used by higher-level wrappers.
///
/// Re-export this so callers of [`crate::textproc`] can implement custom
/// transformations without depending on internal modules.
pub use tokenize::Token;
/// Convenience re-export of [`tokenize::tokenize_markdown`].
#[doc(inline)]
pub use tokenize::tokenize_markdown;

static FENCE_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^\s*(```|~~~).*").unwrap());

static BULLET_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\s*(?:[-*+]|\d+[.)])\s+)(.*)").unwrap());

static FOOTNOTE_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\s*)(\[\^[^]]+\]:\s*)(.*)$").unwrap());

static BLOCKQUOTE_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\s*(?:>\s*)+)(.*)$").unwrap());

/// Matches `markdownlint` comment directives.
///
/// The regex is case-insensitive and recognises these forms with optional rule
/// names (including plugin rules such as `MD013/line-length` or
/// `plugin/rule-name`):
/// - `<!-- markdownlint-disable -->`
/// - `<!-- markdownlint-enable -->`
/// - `<!-- markdownlint-disable-line MD001 MD005 -->`
/// - `<!-- markdownlint-disable-next-line MD001 MD005 -->`
static MARKDOWNLINT_DIRECTIVE_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s*<!--\s*markdownlint-(?:disable|enable|disable-line|disable-next-line)(?:\s+[A-Za-z0-9_\-/]+)*\s*-->\s*$",
    )
    .expect("valid markdownlint regex")
});

struct PrefixHandler {
    re: &'static std::sync::LazyLock<Regex>,
    is_bq: bool,
    build_prefix: fn(&Captures) -> String,
    rest_group: usize,
}

impl PrefixHandler {
    fn build_bullet_prefix(cap: &Captures) -> String { cap[1].to_string() }

    fn build_footnote_prefix(cap: &Captures) -> String { format!("{}{}", &cap[1], &cap[2]) }

    fn build_blockquote_prefix(cap: &Captures) -> String { cap[1].to_string() }
}

static HANDLERS: &[PrefixHandler] = &[
    PrefixHandler {
        re: &BULLET_RE,
        is_bq: false,
        build_prefix: PrefixHandler::build_bullet_prefix,
        rest_group: 2,
    },
    PrefixHandler {
        re: &FOOTNOTE_RE,
        is_bq: false,
        build_prefix: PrefixHandler::build_footnote_prefix,
        rest_group: 3,
    },
    PrefixHandler {
        re: &BLOCKQUOTE_RE,
        is_bq: true,
        build_prefix: PrefixHandler::build_blockquote_prefix,
        rest_group: 2,
    },
];

fn wrap_preserving_code(text: &str, width: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    let mut last_split: Option<usize> = None;
    let tokens = tokenize::segment_inline(text);
    let mut i = 0;
    while i < tokens.len() {
        let mut j = i + 1;
        let mut group_width = UnicodeWidthStr::width(tokens[i].as_str());

        if tokens[i].contains("](") && tokens[i].ends_with(')') {
            while j < tokens.len()
                && tokens[j].chars().all(|c| {
                    matches!(
                        c,
                        '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '"' | '\''
                    )
                })
            {
                group_width += UnicodeWidthStr::width(tokens[j].as_str());
                j += 1;
            }
        }

        if current.is_empty()
            && tokens[i].len() == 1
            && ".?!,:;".contains(tokens[i].as_str())
            && lines
                .last()
                .is_some_and(|l: &String| l.trim_end().ends_with('`'))
        {
            lines
                .last_mut()
                .expect("checked last line exists")
                .push_str(&tokens[i]);
            i += 1;
            continue;
        }

        if current_width + group_width <= width {
            for tok in &tokens[i..j] {
                current.push_str(tok);
                if tok.chars().all(char::is_whitespace) {
                    last_split = Some(current.len());
                }
                current_width += UnicodeWidthStr::width(tok.as_str());
            }
            i = j;
            continue;
        }

        if current_width + group_width > width && last_split.is_some() {
            let pos = last_split.unwrap();
            let line = current[..pos].to_string();
            let mut rest = current[pos..].trim_start().to_string();
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                lines.push(trimmed.to_string());
            }
            for tok in &tokens[i..j] {
                rest.push_str(tok);
            }
            current = rest;
            current_width = UnicodeWidthStr::width(current.as_str());
            last_split = if tokens[j - 1].chars().all(char::is_whitespace) {
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
            i = j;
            continue;
        }

        let trimmed = current.trim_end();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
        current.clear();
        current_width = 0;
        last_split = None;

        for tok in &tokens[i..j] {
            if !tok.chars().all(char::is_whitespace) {
                current.push_str(tok);
                current_width += UnicodeWidthStr::width(tok.as_str());
            }
        }
        if j > i && tokens[j - 1].chars().all(char::is_whitespace) {
            last_split = Some(current.len());
        }
        i = j;
    }
    let trimmed = current.trim_end();
    if !trimmed.is_empty() {
        lines.push(trimmed.to_string());
    }
    lines
}

#[doc(hidden)]
pub fn is_fence(line: &str) -> bool { FENCE_RE.is_match(line) }

pub(crate) fn is_markdownlint_directive(line: &str) -> bool {
    MARKDOWNLINT_DIRECTIVE_RE.is_match(line)
}

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

    'line_loop: for line in lines {
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

        if is_markdownlint_directive(line) {
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

        for handler in HANDLERS {
            if let Some(cap) = handler.re.captures(line) {
                let prefix = (handler.build_prefix)(&cap);
                let rest = cap.get(handler.rest_group).unwrap().as_str();
                handle_prefix_line(
                    &mut out,
                    &mut buf,
                    &mut indent,
                    width,
                    &prefix,
                    rest,
                    handler.is_bq,
                );
                continue 'line_loop;
            }
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
