//! Utilities for wrapping Markdown lines.
//!
//! These helpers reflow paragraphs and list items while preserving inline code
//! spans, fenced code blocks, and other prefixes. Width calculations rely on
//! `UnicodeWidthStr::width` from the `unicode-width` crate as described in
//! `docs/architecture.md#unicode-width-handling`.
//!
//! The [`Token`] enum and [`tokenize_markdown`] function are public so callers
//! can perform custom token-based processing.

use regex::Regex;
use unicode_width::UnicodeWidthStr;

mod fence;
mod tokenize;

pub use fence::is_fence;
/// Token emitted by the `tokenize::segment_inline` parser and used by
/// higher-level wrappers.
///
/// Downstream callers inspect [`Token<'a>`] when implementing bespoke
/// wrapping logic. The `'a` lifetime parameter ties each token to the source
/// text, avoiding unnecessary allocation.
///
/// Re-export these so callers of [`crate::textproc`] can implement custom
/// transformations without depending on internal modules.
pub use tokenize::Token;
#[doc(inline)]
pub use tokenize::tokenize_markdown;

// Permit GFM task list markers with flexible spacing and missing post-marker
// spaces in Markdown.
static BULLET_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(\s*(?:[-*+]|\d+[.)])\s+(?:\[\s*(?:[xX]|\s)\s*\]\s*)?)(.*)",
    "bullet pattern regex should compile",
);

static FOOTNOTE_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(\s*)(\[\^[^]]+\]:\s*)(.*)$",
    "footnote pattern regex should compile",
);

static BLOCKQUOTE_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(\s*(?:>\s*)+)(.*)$",
    "blockquote pattern regex should compile",
);

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

#[inline]
fn is_trailing_punct(c: char) -> bool {
    // ASCII closers + common Unicode closers and word-final punctuation
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '"' | '\''
    ) || "…—–»›）］】》」』、。，：；！？”.’".contains(c)
}

fn extend_punctuation(tokens: &[String], mut j: usize, width: &mut usize) -> usize {
    use unicode_width::UnicodeWidthStr;
    while j < tokens.len() && tokens[j].chars().all(is_trailing_punct) {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
    }
    j
}

#[inline]
fn merge_code_span(tokens: &[String], i: usize, width: &mut usize) -> usize {
    use unicode_width::UnicodeWidthStr;
    debug_assert!(
        tokens[i] == "`",
        "merge_code_span requires a single backtick opener"
    );
    let mut j = i + 1;
    while j < tokens.len() && tokens[j] != "`" {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
    }
    if j < tokens.len() {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
        j = extend_punctuation(tokens, j, width);
    }
    j
}

#[inline]
fn flush_current(lines: &mut Vec<String>, current: &mut String) {
    let cap = current.capacity();
    lines.push(std::mem::take(current));
    *current = String::with_capacity(cap);
}

fn flush_trailing_whitespace(lines: &mut Vec<String>, current: &mut String, token: &str) {
    debug_assert!(
        token.chars().all(char::is_whitespace),
        "expected whitespace token",
    );
    current.push_str(token);
    flush_current(lines, current);
}

fn determine_token_span(tokens: &[String], start: usize) -> (usize, usize) {
    let mut end = start + 1;
    let mut width = UnicodeWidthStr::width(tokens[start].as_str());

    if tokens[start] == "`" {
        end = merge_code_span(tokens, start, &mut width);
    }

    if tokens[start].contains("](") && tokens[start].ends_with(')') {
        end = extend_punctuation(tokens, end, &mut width);
    }

    if tokens[start].starts_with('`') && tokens[start].ends_with('`') {
        end = extend_punctuation(tokens, end, &mut width);

        while end < tokens.len() && tokens[end].starts_with('`') && tokens[end].ends_with('`') {
            width += UnicodeWidthStr::width(tokens[end].as_str());
            end += 1;
            end = extend_punctuation(tokens, end, &mut width);
        }
    }

    (end, width)
}

fn attach_punctuation_to_previous_line(lines: &mut [String], current: &str, token: &str) -> bool {
    if !current.is_empty() || token.len() != 1 || !".?!,:;".contains(token) {
        return false;
    }

    if let Some(last_line) = lines.last_mut()
        && last_line.trim_end().ends_with('`')
    {
        last_line.push_str(token);
        return true;
    }

    false
}

fn extend_current_with_group(
    tokens: &[String],
    start: usize,
    end: usize,
    current: &mut String,
    current_width: &mut usize,
    last_split: &mut Option<usize>,
) {
    for tok in &tokens[start..end] {
        let token_str = tok.as_str();
        if token_str.len() == 1 && ".?!,:;".contains(token_str) && current.trim_end().ends_with('`')
        {
            let trimmed_len = current.trim_end_matches(char::is_whitespace).len();
            if trimmed_len < current.len() {
                let removed_segment = current[trimmed_len..].to_string();
                let removed_width = UnicodeWidthStr::width(removed_segment.as_str());
                current.truncate(trimmed_len);
                *current_width = current_width.saturating_sub(removed_width);
                *last_split = current
                    .char_indices()
                    .rev()
                    .find(|(_, ch)| ch.is_whitespace())
                    .map(|(idx, ch)| idx + ch.len_utf8());
            }
        }

        current.push_str(token_str);
        if tok.chars().all(char::is_whitespace) {
            *last_split = Some(current.len());
        }
        *current_width += UnicodeWidthStr::width(token_str);
    }
}

fn wrap_preserving_code(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    let mut last_split: Option<usize> = None;
    let tokens = tokenize::segment_inline(text);
    let mut i = 0;
    while i < tokens.len() {
        let (group_end, group_width) = determine_token_span(&tokens, i);
        if attach_punctuation_to_previous_line(lines.as_mut_slice(), &current, &tokens[i]) {
            i += 1;
            continue;
        }

        if current_width + group_width <= width {
            extend_current_with_group(
                &tokens,
                i,
                group_end,
                &mut current,
                &mut current_width,
                &mut last_split,
            );
            i = group_end;
            continue;
        }

        if last_split.is_some() {
            let pos = last_split.expect("split position exists");
            let line = current[..pos].to_string();
            let mut rest = current[pos..].trim_start().to_string();
            if !line.trim_end().is_empty() {
                let trimmed = line.trim_end();
                let whitespace_segment = &line[trimmed.len()..];
                if whitespace_segment.chars().count() > 1 {
                    // Preserve multi-space runs so Markdown hard breaks survive wrapping.
                    lines.push(line);
                } else {
                    lines.push(trimmed.to_string());
                }
            }
            for tok in &tokens[i..group_end] {
                rest.push_str(tok);
            }
            current = rest;
            current_width = UnicodeWidthStr::width(current.as_str());
            last_split = if group_end > i && tokens[group_end - 1].chars().all(char::is_whitespace)
            {
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
            i = group_end;
            continue;
        }

        if tokens[i].chars().all(char::is_whitespace) && group_end == tokens.len() {
            if !current.is_empty() {
                flush_trailing_whitespace(&mut lines, &mut current, &tokens[i]);
            }
            current_width = 0;
            last_split = None;
            i = group_end;
            continue;
        }

        if !current.is_empty() {
            flush_current(&mut lines, &mut current);
        }
        current_width = 0;
        last_split = None;

        for tok in &tokens[i..group_end] {
            if !tok.chars().all(char::is_whitespace) {
                current.push_str(tok);
                current_width += UnicodeWidthStr::width(tok.as_str());
            }
        }
        if group_end > i && tokens[group_end - 1].chars().all(char::is_whitespace) {
            last_split = Some(current.len());
        }
        i = group_end;
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

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
    // Track the currently open fence: (marker char, run length), e.g., ('`', 4) or ('~', 3).
    let mut fence_state: Option<(char, usize)> = None;

    for line in lines {
        if fence::handle_fence_line(
            &mut out,
            &mut buf,
            &mut indent,
            width,
            line,
            &mut in_code,
            &mut fence_state,
        ) {
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

        if let Some(cap) = BULLET_RE.captures(line) {
            let prefix = cap.get(1).expect("bullet regex capture").as_str();
            let rest = cap.get(2).expect("bullet regex remainder capture").as_str();
            handle_prefix_line(&mut out, &mut buf, &mut indent, width, prefix, rest, false);
            continue;
        }

        if let Some(cap) = FOOTNOTE_RE.captures(line) {
            let prefix = format!("{}{}", &cap[1], &cap[2]);
            let rest = cap
                .get(3)
                .expect("footnote regex remainder capture")
                .as_str();
            handle_prefix_line(&mut out, &mut buf, &mut indent, width, &prefix, rest, false);
            continue;
        }

        if let Some(cap) = BLOCKQUOTE_RE.captures(line) {
            let prefix = cap.get(1).expect("blockquote prefix capture").as_str();
            let rest = cap
                .get(2)
                .expect("blockquote regex remainder capture")
                .as_str();
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
