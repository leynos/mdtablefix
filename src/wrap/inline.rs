//! Inline wrapping helpers that keep code spans intact.
//!
//! These functions operate on token streams so `wrap_text` can preserve
//! inline code, links, and trailing punctuation without reimplementing the
//! grouping logic in multiple places.

use unicode_width::UnicodeWidthStr;

use super::{line_buffer::LineBuffer, tokenize};

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

pub(super) fn determine_token_span(tokens: &[String], start: usize) -> (usize, usize) {
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

pub(super) fn attach_punctuation_to_previous_line(
    lines: &mut [String],
    current: &str,
    token: &str,
) -> bool {
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

fn push_span_with_carry(
    buffer: &mut LineBuffer,
    tokens: &[String],
    start: usize,
    end: usize,
    carried_whitespace: &mut String,
) {
    if start >= end {
        return;
    }

    if carried_whitespace.is_empty() {
        buffer.push_span(tokens, start, end);
        return;
    }

    let mut first_token = std::mem::take(carried_whitespace);
    first_token.push_str(tokens[start].as_str());
    buffer.push_token(first_token.as_str());
    if start + 1 < end {
        buffer.push_span(tokens, start + 1, end);
    }
}

pub(super) fn wrap_preserving_code(text: &str, width: usize) -> Vec<String> {
    let tokens = tokenize::segment_inline(text);
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut buffer = LineBuffer::new();
    let mut carried_whitespace = String::new();
    let mut i = 0;

    while i < tokens.len() {
        let (group_end, group_width) = determine_token_span(&tokens, i);
        let span_is_whitespace = tokens[i..group_end]
            .iter()
            .all(|tok| is_whitespace_token(tok));

        if span_is_whitespace && !carried_whitespace.is_empty() && group_end != tokens.len() {
            for tok in &tokens[i..group_end] {
                carried_whitespace.push_str(tok);
            }
            i = group_end;
            continue;
        }

        if attach_punctuation_to_previous_line(lines.as_mut_slice(), buffer.text(), &tokens[i]) {
            carried_whitespace.clear();
            i += 1;
            continue;
        }

        if buffer.width() + group_width <= width {
            push_span_with_carry(&mut buffer, &tokens, i, group_end, &mut carried_whitespace);
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
        if span_is_whitespace {
            for tok in &tokens[i..group_end] {
                carried_whitespace.push_str(tok);
            }
            i = group_end;
            continue;
        }

        push_span_with_carry(&mut buffer, &tokens, i, group_end, &mut carried_whitespace);
        i = group_end;
    }

    if !carried_whitespace.is_empty() {
        buffer.push_token(carried_whitespace.as_str());
        carried_whitespace.clear();
    }

    buffer.flush_into(&mut lines);
    lines
}
