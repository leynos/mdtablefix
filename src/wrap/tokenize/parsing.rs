//! Higher-level inline Markdown parsing helpers, isolated from tokenizer entry points.

use tracing::{debug, trace};

use super::scanning::{collect_range, position_after_close, scan_while};

/// Parse a Markdown link or image starting at `i`.
///
/// Recognizes GFM footnote references of the form `[^label]` when they are not
/// followed by a URL. Caret-labelled links such as `[^label](url)` remain links.
///
/// Tracks nested URL parentheses and returns the parsed slice plus its end index.
/// Instrumentation records content-free metadata without exposing document text.
///
/// # Examples
///
/// ```rust,ignore
/// let text = "![alt](a(b)c)";
/// let (tok, idx) = parse_link_or_image(text, 0);
/// assert_eq!(tok, "![alt](a(b)c)");
/// assert_eq!(idx, text.len());
/// ```
#[tracing::instrument(level = "debug", skip(text))]
pub(super) fn parse_link_or_image(text: &str, mut idx: usize) -> (String, usize) {
    let start = idx;

    if let Some(text_end) = find_footnote_end(text, idx)
        && (text_end == text.len() || !text[text_end..].starts_with('('))
    {
        if tracing::enabled!(tracing::Level::DEBUG) {
            debug!(
                token_length = text[start..text_end].chars().count(),
                "footnote reference parsed"
            );
        }
        return (collect_range(text, start, text_end), text_end);
    }

    if text[idx..].starts_with('!') {
        idx += '!'.len_utf8();
    }

    let Some(text_end) = parse_link_text(text, idx) else {
        return fallback_single_char(text, start);
    };

    if text_end < text.len() && text[text_end..].starts_with('(') {
        if let Some(url_end) = parse_link_url(text, text_end) {
            if tracing::enabled!(tracing::Level::DEBUG) {
                let is_image = text[start..].starts_with('!');
                debug!(
                    token_length = text[start..url_end].chars().count(),
                    is_image, "link or image parsed"
                );
            }
            return (collect_range(text, start, url_end), url_end);
        }
        // Unbalanced URL: mirror the original behaviour by returning
        // everything through the end of the string.
        return (collect_range(text, start, text.len()), text.len());
    }

    if text_end < text.len()
        && text[text_end..].starts_with('[')
        && let Some(reference_end) = parse_link_text(text, text_end)
    {
        return (collect_range(text, start, reference_end), reference_end);
    }

    fallback_single_char(text, start)
}

#[tracing::instrument(level = "trace", skip(text), ret)]
fn find_footnote_end(text: &str, idx: usize) -> Option<usize> {
    if idx >= text.len() || !text[idx..].starts_with("[^") {
        if tracing::enabled!(tracing::Level::TRACE) {
            trace!(
                start = idx,
                reason = "prefix_mismatch",
                "footnote end not found"
            );
        }
        return None;
    }

    let mut cursor = idx + "[^".len();
    while cursor < text.len() {
        let ch = text[cursor..].chars().next()?;
        cursor += ch.len_utf8();

        if ch == '\\' {
            if let Some(escaped) = text[cursor..].chars().next() {
                cursor += escaped.len_utf8();
            }
            continue;
        }

        if ch == ']' {
            if tracing::enabled!(tracing::Level::TRACE) {
                trace!(
                    start = idx,
                    end = cursor,
                    token_length = text[idx..cursor].chars().count(),
                    "footnote label span recognized"
                );
            }
            return Some(cursor);
        }
    }

    if tracing::enabled!(tracing::Level::TRACE) {
        trace!(
            start = idx,
            reason = "unterminated_bracket",
            "footnote end not found"
        );
    }
    None
}

pub(super) fn parse_link_text(text: &str, idx: usize) -> Option<usize> {
    if idx >= text.len() || !text[idx..].starts_with('[') {
        return None;
    }
    let mut cursor = idx + '['.len_utf8();
    let mut preceding_backslash_is_odd = false;
    while cursor < text.len() {
        let ch = text[cursor..].chars().next()?;
        cursor += ch.len_utf8();
        if ch == ']' && !preceding_backslash_is_odd {
            return Some(cursor);
        }
        preceding_backslash_is_odd = ch == '\\' && !preceding_backslash_is_odd;
    }
    None
}

pub(super) fn parse_link_url(text: &str, mut idx: usize) -> Option<usize> {
    if idx >= text.len() || !text[idx..].starts_with('(') {
        return None;
    }
    idx += '('.len_utf8();
    let mut depth = 1;
    let mut preceding_backslash_is_odd = false;
    while idx < text.len() {
        let Some(ch) = text[idx..].chars().next() else {
            break;
        };
        let is_escaped = preceding_backslash_is_odd;
        idx += ch.len_utf8();
        match ch {
            '(' if !is_escaped => depth += 1,
            ')' if !is_escaped => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
        preceding_backslash_is_odd = ch == '\\' && !preceding_backslash_is_odd;
    }
    None
}

fn fallback_single_char(text: &str, start: usize) -> (String, usize) {
    let next = text[start..]
        .chars()
        .next()
        .map_or(text.len(), |ch| start + ch.len_utf8());
    (collect_range(text, start, next), next)
}

/// Determine whether the character at `idx` begins a Markdown image literal.
///
/// # Examples
///
/// ```rust,ignore
/// assert!(looks_like_image_start("![alt](url)", 0, '!'));
/// assert!(!looks_like_image_start("! not", 0, '!'));
/// ```
pub(super) fn looks_like_image_start(text: &str, idx: usize, ch: char) -> bool {
    if ch != '!' {
        return false;
    }
    let after_bang = idx + ch.len_utf8();
    after_bang <= text.len() && text[after_bang..].starts_with('[')
}

/// Determine whether a character is considered trailing punctuation.
///
/// The wrapper treats such punctuation as part of the preceding link when
/// wrapping lines.
///
/// # Examples
///
/// ```rust,ignore
/// assert!(is_trailing_punctuation('.'));
/// assert!(is_trailing_punctuation('('));
/// assert!(!is_trailing_punctuation('a'));
/// ```
pub(super) fn is_trailing_punctuation(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | '(' | ')' | ']' | '"' | '\''
    )
}

pub(super) fn handle_backtick_fence(text: &str, start_idx: usize) -> (String, usize) {
    let start = start_idx;
    let fence_end = scan_while(text, start_idx, |ch| ch == '`');
    let fence_len = fence_end - start;

    if let Some(candidate_end) = position_after_close(text, fence_end, fence_len) {
        return (collect_range(text, start, candidate_end), candidate_end);
    }

    (collect_range(text, start, fence_end), fence_end)
}

#[cfg(test)]
#[path = "parsing_tests.rs"]
mod tests;
