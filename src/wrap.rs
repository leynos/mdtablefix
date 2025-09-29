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
mod line_buffer;
mod tokenize;
pub(crate) use self::line_buffer::LineBuffer;
pub use fence::{FenceTracker, is_fence};
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

fn looks_like_link(token: &str) -> bool {
    (token.starts_with('[') || token.starts_with("!["))
        && token.contains("](")
        && token.ends_with(')')
}

fn is_whitespace_token(token: &str) -> bool {
    token.chars().all(char::is_whitespace)
}

fn is_inline_code_token(token: &str) -> bool {
    token.starts_with('`') && token.ends_with('`')
}

fn extend_punctuation(tokens: &[String], mut j: usize, width: &mut usize) -> usize {
    while j < tokens.len() && tokens[j].chars().all(is_trailing_punct) {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
    }
    j
}

#[inline]
fn merge_code_span(tokens: &[String], i: usize, width: &mut usize) -> usize {
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
fn determine_token_span(tokens: &[String], start: usize) -> (usize, usize) {
    #[derive(PartialEq, Eq)]
    enum SpanKind {
        General,
        Code,
        Link,
    }

    let mut end = start + 1;
    let mut width = UnicodeWidthStr::width(tokens[start].as_str());
    let mut kind = SpanKind::General;

    if tokens[start] == "`" {
        kind = SpanKind::Code;
        end = merge_code_span(tokens, start, &mut width);
    } else if is_inline_code_token(&tokens[start]) {
        kind = SpanKind::Code;
        end = extend_punctuation(tokens, end, &mut width);
    } else if looks_like_link(&tokens[start]) {
        kind = SpanKind::Link;
        end = extend_punctuation(tokens, end, &mut width);
    }

    while end < tokens.len() {
        let token = &tokens[end];
        if is_whitespace_token(token) {
            if matches!(kind, SpanKind::Code | SpanKind::Link)
                && end + 1 < tokens.len()
                && (looks_like_link(&tokens[end + 1])
                    || is_inline_code_token(&tokens[end + 1])
                    || tokens[end + 1].chars().all(is_trailing_punct))
            {
                width += UnicodeWidthStr::width(token.as_str());
                end += 1;
                continue;
            }
            break;
        }

        if token.chars().all(is_trailing_punct) {
            if matches!(kind, SpanKind::Code | SpanKind::Link) {
                width += UnicodeWidthStr::width(token.as_str());
                end += 1;
                continue;
            }
            break;
        }

        let is_link = looks_like_link(token);
        let is_code = is_inline_code_token(token);

        if kind == SpanKind::Link && is_link {
            width += UnicodeWidthStr::width(token.as_str());
            end += 1;
            end = extend_punctuation(tokens, end, &mut width);
            continue;
        }

        if kind == SpanKind::Code && is_code {
            width += UnicodeWidthStr::width(token.as_str());
            end += 1;
            end = extend_punctuation(tokens, end, &mut width);
            continue;
        }

        break;
    }

    (end, width)
}

fn attach_punctuation_to_previous_line(lines: &mut [String], current: &str, token: &str) -> bool {
    if !current.is_empty() || token.len() != 1 || !".?!,:;".contains(token) {
        return false;
    }

    let Some(last_line) = lines.last_mut() else {
        return false;
    };

    if last_line.trim_end().ends_with('`') {
        last_line.push_str(token);
        return true;
    }

    false
}

fn wrap_preserving_code(text: &str, width: usize) -> Vec<String> {
    let tokens = tokenize::segment_inline(text);
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut buffer = LineBuffer::new();
    let mut i = 0;

    while i < tokens.len() {
        let (group_end, group_width) = determine_token_span(&tokens, i);

        if attach_punctuation_to_previous_line(lines.as_mut_slice(), buffer.text(), &tokens[i]) {
            i += 1;
            continue;
        }

        if buffer.width() + group_width <= width {
            buffer.push_span(&tokens, i, group_end);
            i = group_end;
            continue;
        }

        if buffer.split_with_span(&mut lines, &tokens, i, group_end, width) {
            i = group_end;
            continue;
        }

        if buffer.flush_trailing_whitespace(&mut lines, &tokens, i, group_end) {
            i = group_end;
            continue;
        }

        buffer.flush_into(&mut lines);
        buffer.push_non_whitespace_span(&tokens, i, group_end);
        i = group_end;
    }

    buffer.flush_into(&mut lines);
    lines
}

/// Describes the Markdown block prefix detected by [`classify_block`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockKind {
    /// Lines that begin with `#`, `##`, and similar heading prefixes.
    Heading,
    /// Bullet or ordered list markers matched by [`BULLET_RE`].
    Bullet,
    /// Lines that begin with one or more `>` markers.
    Blockquote,
    /// Footnote definitions recognised by [`FOOTNOTE_RE`].
    FootnoteDefinition,
    /// HTML-style markdownlint directives recognised by [`is_markdownlint_directive`].
    MarkdownlintDirective,
    /// Lines whose first non-whitespace character is an ASCII digit.
    DigitPrefix,
}

/// Classifies block-level Markdown prefixes shared by wrapping and table detection.
///
/// Detection order determines precedence when a line could match multiple prefixes.
/// The current precedence is: heading, bullet, blockquote, footnote definition,
/// markdownlint directive, digit prefix. Headings outrank bullets and blockquotes,
/// so inputs such as "# 1" remain headings rather than list items. Headings ignore
/// indentation of four or more spaces so indented code remains untouched.
/// For example, passing `"> quote"` returns `Some(BlockKind::Blockquote)` while
/// `"| cell |"` yields `None` because the line is part of a table.
pub(crate) fn classify_block(line: &str) -> Option<BlockKind> {
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len());

    if indent < 4 && trimmed.starts_with('#') {
        return Some(BlockKind::Heading);
    }
    if BULLET_RE.is_match(line) {
        return Some(BlockKind::Bullet);
    }
    if BLOCKQUOTE_RE.is_match(line) {
        return Some(BlockKind::Blockquote);
    }
    if FOOTNOTE_RE.is_match(line) {
        return Some(BlockKind::FootnoteDefinition);
    }
    if is_markdownlint_directive(line) {
        return Some(BlockKind::MarkdownlintDirective);
    }
    if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Some(BlockKind::DigitPrefix);
    }
    None
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

fn is_indented_code_line(line: &str) -> bool {
    let indent_width = line
        .as_bytes()
        .iter()
        .take_while(|b| **b == b' ' || **b == 0x09)
        .fold(0_usize, |acc, &b| acc + if b == 0x09 { 4 } else { 1 });

    indent_width >= 4 && line.chars().any(|c| !c.is_whitespace())
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
    // Track fenced code blocks so wrapping honours shared fence semantics.
    let mut fence_tracker = FenceTracker::default();

    for line in lines {
        if fence::handle_fence_line(
            &mut out,
            &mut buf,
            &mut indent,
            width,
            line,
            &mut fence_tracker,
        ) {
            continue;
        }

        if fence_tracker.in_fence() {
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

        if matches!(
            classify_block(line),
            Some(BlockKind::Heading | BlockKind::MarkdownlintDirective)
        ) {
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

        if is_indented_code_line(line) {
            // Preserve indented code blocks verbatim so wrapping does not merge them into paragraphs.
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            out.push(line.clone());
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
