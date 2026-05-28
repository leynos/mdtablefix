//! Higher-level parsing helpers for inline Markdown elements.
//!
//! Splitting these routines from the main tokenizer keeps `mod.rs` focused on
//! public API surface area while giving the parsing logic a contained space for
//! documentation and direct unit tests.

use tracing::{debug, trace};

use super::scanning::{collect_range, has_odd_backslash_escape_bytes, scan_while};

/// Parse a Markdown link or image starting at `i`.
///
/// Recognizes GFM footnote references of the form `[^label]` and returns them
/// immediately when they are not followed by a URL. Caret-labelled links with
/// a following URL, such as `[^label](url)`, are still parsed as normal links.
///
/// Handles nested parentheses within URLs by tracking the depth of opening and
/// closing delimiters. Returns the parsed slice and the index after the closing
/// parenthesis if one is found.
///
/// The `#[tracing::instrument]` attribute records the entry, arguments, and
/// return value automatically so callers can observe classification decisions
/// without the function body managing its own span events.
///
/// # Examples
///
/// ```rust,ignore
/// let text = "![alt](a(b)c)";
/// let (tok, idx) = parse_link_or_image(text, 0);
/// assert_eq!(tok, "![alt](a(b)c)");
/// assert_eq!(idx, text.len());
/// ```
#[tracing::instrument(level = "debug", skip(text), ret)]
pub(super) fn parse_link_or_image(text: &str, mut idx: usize) -> (String, usize) {
    let start = idx;

    if let Some(text_end) = find_footnote_end(text, idx)
        && (text_end == text.len() || !text[text_end..].starts_with('('))
    {
        if tracing::enabled!(tracing::Level::DEBUG) {
            debug!(token = %&text[start..text_end], "footnote reference parsed");
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
                debug!(token = %&text[start..url_end], is_image, "link or image parsed");
            }
            return (collect_range(text, start, url_end), url_end);
        }
        // Unbalanced URL: mirror the original behaviour by returning
        // everything through the end of the string.
        return (collect_range(text, start, text.len()), text.len());
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
                    token = %&text[idx..cursor],
                    "footnote label span recognised"
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

fn parse_link_text(text: &str, idx: usize) -> Option<usize> {
    if idx >= text.len() || !text[idx..].starts_with('[') {
        return None;
    }
    let mut cursor = idx + '['.len_utf8();
    cursor = scan_while(text, cursor, |c| c != ']');
    if cursor < text.len() && text[cursor..].starts_with(']') {
        Some(cursor + ']'.len_utf8())
    } else {
        None
    }
}

fn parse_link_url(text: &str, mut idx: usize) -> Option<usize> {
    if idx >= text.len() || !text[idx..].starts_with('(') {
        return None;
    }
    idx += '('.len_utf8();
    let mut depth = 1;
    while idx < text.len() {
        let Some(ch) = text[idx..].chars().next() else {
            break;
        };
        idx += ch.len_utf8();
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
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
    use proptest::prelude::*;

    use super::*;

    fn footnote_label_part_strategy() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop_oneof![
                (b'a'..=b'z').prop_map(char::from),
                (b'A'..=b'Z').prop_map(char::from),
                (b'0'..=b'9').prop_map(char::from),
                Just('-'),
                Just('_')
            ],
            0..12,
        )
        .prop_map(|chars| chars.into_iter().collect())
    }

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

    #[test]
    fn parse_link_or_image_handles_deeply_nested_parentheses() {
        let text = "[link](url(a(b(c)d)e)) tail";
        let (token, idx) = parse_link_or_image(text, 0);
        assert_eq!(token, "[link](url(a(b(c)d)e))");
        assert_eq!(idx, token.len());
    }

    #[test]
    fn parse_link_or_image_handles_nested_parentheses_for_images() {
        let text = "![alt](path(a(b(c)d)e))";
        let (token, idx) = parse_link_or_image(text, 0);
        assert_eq!(token, "![alt](path(a(b(c)d)e))");
        assert_eq!(idx, token.len());
    }

    #[test]
    fn parse_link_or_image_handles_text_ending_at_bracket() {
        let text = "[";
        let (token, idx) = parse_link_or_image(text, 0);
        assert_eq!(token, "[");
        assert_eq!(idx, 1);
    }

    #[test]
    fn parse_link_or_image_preserves_footnote_reference() {
        let text = "[^4] tail";
        let (token, idx) = parse_link_or_image(text, 0);
        assert_eq!(token, "[^4]");
        assert_eq!(idx, token.len());
    }

    #[test]
    fn parse_link_or_image_preserves_footnote_reference_with_escaped_bracket() {
        let text = r"[^a\]b] tail";
        let (token, idx) = parse_link_or_image(text, 0);
        assert_eq!(token, r"[^a\]b]");
        assert_eq!(idx, token.len());
    }

    #[test]
    fn parse_link_or_image_preserves_footnote_at_end() {
        let text = "[^4]";
        let (token, idx) = parse_link_or_image(text, 0);
        assert_eq!(token, "[^4]");
        assert_eq!(idx, token.len());
    }

    #[test]
    fn parse_link_or_image_keeps_caret_text_links_as_links() {
        let text = "[^label](https://example.com) tail";
        let (token, idx) = parse_link_or_image(text, 0);
        assert_eq!(token, "[^label](https://example.com)");
        assert_eq!(idx, token.len());
    }

    proptest! {
        #[test]
        fn parse_link_or_image_preserves_footnote_references_with_escaped_brackets(
            prefix in footnote_label_part_strategy(),
            suffix in footnote_label_part_strategy(),
        ) {
            let expected = format!(r"[^{prefix}\]{suffix}]");
            let expected_len = expected.len();
            let text = format!("{expected} tail");

            let (token, idx) = parse_link_or_image(&text, 0);

            prop_assert_eq!(token, expected);
            prop_assert_eq!(idx, expected_len);
        }
    }

    mod tracing_tests {
        use tracing_test::traced_test;

        use super::*;

        #[traced_test]
        #[test]
        fn parse_link_or_image_logs_footnote_reference() {
            let _ = parse_link_or_image("[^4] tail", 0);
            assert!(logs_contain("footnote reference parsed"));
        }

        #[traced_test]
        #[test]
        fn parse_link_or_image_logs_link_parsed() {
            let _ = parse_link_or_image("[link](url)", 0);
            assert!(logs_contain("link or image parsed"));
        }

        #[traced_test]
        #[test]
        fn find_footnote_end_logs_prefix_mismatch() {
            let _ = find_footnote_end("no-caret", 0);
            assert!(logs_contain("prefix_mismatch"));
        }
    }
}
