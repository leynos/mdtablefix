//! Higher-level parsing helpers for inline Markdown elements.
//!
//! Splitting these routines from the main tokenizer keeps `mod.rs` focused on
//! public API surface area while giving the parsing logic a contained space for
//! documentation and direct unit tests.

use super::scanning::{collect_range, has_odd_backslash_escape_bytes, scan_while};

/// Parse a Markdown link or image starting at `i`.
///
/// Handles nested parentheses within URLs by tracking the depth of opening and
/// closing delimiters. Returns the parsed slice and the index after the closing
/// parenthesis if one is found.
///
/// # Examples
///
/// ```rust,ignore
/// let text = "![alt](a(b)c)";
/// let (tok, idx) = parse_link_or_image(text, 0);
/// assert_eq!(tok, "![alt](a(b)c)");
/// assert_eq!(idx, text.len());
/// ```
pub(super) fn parse_link_or_image(text: &str, mut idx: usize) -> (String, usize) {
    let start = idx;

    if text[idx..].starts_with('!') {
        idx += '!'.len_utf8();
    }

    if !text[idx..].starts_with('[') {
        let next = text[start..]
            .chars()
            .next()
            .map_or(text.len(), |ch| start + ch.len_utf8());
        return (collect_range(text, start, next), next);
    }

    idx += '['.len_utf8();
    idx = scan_while(text, idx, |c| c != ']');
    if idx < text.len() && text[idx..].starts_with(']') {
        idx += ']'.len_utf8();
        if idx < text.len() && text[idx..].starts_with('(') {
            idx += '('.len_utf8();
            let mut depth = 1;
            while idx < text.len() && depth > 0 {
                if let Some(ch) = text[idx..].chars().next() {
                    idx += ch.len_utf8();
                    match ch {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                } else {
                    break;
                }
            }
            return (collect_range(text, start, idx), idx);
        }
    }

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

pub(super) fn handle_backtick_fence(text: &str, bytes: &[u8], start_idx: usize) -> (String, usize) {
    let start = start_idx;
    let fence_end = scan_while(text, start_idx, |ch| ch == '`');
    let fence_len = fence_end - start;
    let mut end = fence_end;

    while end < text.len() {
        let candidate_end = scan_while(text, end, |ch| ch == '`');
        if candidate_end - end == fence_len && !has_odd_backslash_escape_bytes(bytes, end) {
            return (collect_range(text, start, candidate_end), candidate_end);
        }

        if let Some(next) = text[end..].chars().next() {
            end += next.len_utf8();
        } else {
            break;
        }
    }

    (collect_range(text, start, fence_end), fence_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_link_or_image_handles_nested_parentheses() {
        let text = "![alt](path(a(b)c)) more";
        let (token, idx) = parse_link_or_image(text, 0);
        assert_eq!(token, "![alt](path(a(b)c))");
        assert_eq!(idx, token.len());
    }

    #[test]
    fn parse_link_or_image_falls_back_on_malformed_input() {
        let text = "[broken";
        let (token, idx) = parse_link_or_image(text, 0);
        assert_eq!(token, "[");
        assert_eq!(idx, "[".len());
    }
}
