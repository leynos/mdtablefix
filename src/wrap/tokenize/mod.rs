//! Tokenization helpers for wrapping logic.
//!
//! This module contains utilities for breaking lines into tokens so that
//! inline code spans and Markdown links are preserved during wrapping.

mod parsing;
mod scanning;

use parsing::{
    handle_backtick_fence, is_trailing_punctuation, looks_like_image_start, parse_link_or_image,
};
use scanning::{
    bracket_follows_escaped_bang, collect_range, has_odd_backslash_escape_bytes, scan_while,
};

/// Markdown token emitted by the `segment_inline` tokenizer.
#[derive(Debug, PartialEq)]
pub enum Token<'a> {
    /// Line within a fenced code block, including the fence itself.
    Fence(&'a str),
    /// Inline code span carrying the original fenced substring.
    Code {
        raw: &'a str,
        fence: &'a str,
        code: &'a str,
    },
    /// Plain text outside code regions.
    Text(&'a str),
    /// Line break separating tokens.
    Newline,
}

/// Break a single line of text into inline token strings.
///
/// Code spans, links, images and surrounding whitespace are preserved as
/// separate tokens. This simplifies later wrapping logic which operates on
/// slices of the original text.
///
/// # Examples
///
/// ```rust,ignore
/// let tokens = segment_inline("see [link](url) and `code`");
/// assert_eq!(
///     tokens,
///     vec!["see", " ", "[link](url)", " ", "and", " ", "`code`"]
/// );
///
/// // Example with consecutive and unusual whitespace
/// let tokens = segment_inline("foo  bar\tbaz   `qux`");
/// assert_eq!(
///     tokens,
///     vec!["foo", "  ", "bar", "\t", "baz", "   ", "`qux`"]
/// );
/// ```
pub(super) fn segment_inline(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < text.len() {
        let Some(ch) = text[i..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            let start = i;
            i = scan_while(text, i, char::is_whitespace);
            tokens.push(collect_range(text, start, i));
            continue;
        } else if ch == '`' {
            if has_odd_backslash_escape_bytes(bytes, i) {
                if let Some(last) = tokens.last_mut() {
                    last.push('`');
                } else {
                    tokens.push(String::from("`"));
                }
                i += ch.len_utf8();
                continue;
            }

            let (token, new_i) = handle_backtick_fence(text, bytes, i);
            tokens.push(token);
            i = new_i;
            continue;
        }

        let looks_like_image = looks_like_image_start(text, i, ch);
        let is_escaped = has_odd_backslash_escape_bytes(bytes, i);
        if (ch == '[' || looks_like_image) && !is_escaped {
            let (tok, mut new_i) = parse_link_or_image(text, i);
            tokens.push(tok);
            let punct_start = new_i;
            new_i = scan_while(text, new_i, is_trailing_punctuation);
            if new_i > punct_start {
                tokens.push(collect_range(text, punct_start, new_i));
            }
            i = new_i;
        } else {
            let start = i;
            while i < text.len() {
                let Some(current) = text[i..].chars().next() else {
                    break;
                };
                if current.is_whitespace() || current == '`' {
                    break;
                }
                let current_escaped = has_odd_backslash_escape_bytes(bytes, i);
                if current == '[' {
                    if !current_escaped && !bracket_follows_escaped_bang(bytes, i) {
                        break;
                    }
                } else if looks_like_image_start(text, i, current) && !current_escaped {
                    break;
                }
                i += current.len_utf8();
            }
            tokens.push(collect_range(text, start, i));
        }
    }
    tokens
}

fn next_token(line: &str, offset: usize) -> Option<(Token<'_>, usize)> {
    if offset >= line.len() {
        return None;
    }

    let rest = &line[offset..];
    if rest.is_empty() {
        return None;
    }

    let bytes = line.as_bytes();
    let delim_len = rest.chars().take_while(|&c| c == '`').count();
    if delim_len == 0 {
        let mut search_offset = 0;
        while let Some(pos) = rest[search_offset..].find('`') {
            let candidate = search_offset + pos;
            if has_odd_backslash_escape_bytes(bytes, offset + candidate) {
                search_offset = candidate + 1;
                continue;
            }
            if candidate == 0 {
                break;
            }
            return Some((Token::Text(&rest[..candidate]), candidate));
        }
        return Some((Token::Text(rest), rest.len()));
    }

    if has_odd_backslash_escape_bytes(bytes, offset) {
        if let Some(first) = rest.chars().next() {
            let used = first.len_utf8();
            return Some((Token::Text(&rest[..used]), used));
        }
        return None;
    }

    let fence = &rest[..delim_len];
    let mut search_start = delim_len;
    while let Some(pos) = rest[search_start..].find(fence) {
        let candidate = search_start + pos;
        if !has_odd_backslash_escape_bytes(bytes, offset + candidate) {
            let raw_end = candidate + delim_len;
            let code = &rest[delim_len..candidate];
            let raw = &rest[..raw_end];
            return Some((Token::Code { raw, fence, code }, raw_end));
        }
        search_start = candidate + 1;
    }

    Some((Token::Text(fence), delim_len))
}

/// Emit [`Token`]s for inline segments within a single line.
///
/// The function scans for backtick sequences and yields `Token::Code` for
/// matched spans. Text outside code spans is emitted as `Token::Text` via the
/// provided callback.
///
/// # Examples
///
/// ```rust,ignore
/// // Prints:
/// // Token::Text("run ")
/// // Token::Code { raw: "`cmd`", fence: "`", code: "cmd" }
/// tokenize_inline("run `cmd`", &mut |t| println!("{:?}", t));
/// ```
///
/// The callback receives each token as a [`Token<'a>`], such as
/// `Token::Text(&str)` or `Token::Code { raw: &str, fence: &str, code: &str }`.
fn tokenize_inline<'a, F>(line: &'a str, mut emit: F)
where
    F: FnMut(Token<'a>),
{
    let mut offset = 0;
    while offset < line.len() {
        if let Some((tok, used)) = next_token(line, offset) {
            emit(tok);
            if used == 0 {
                break;
            }
            offset += used;
        } else {
            break;
        }
    }
}

/// Tokenize a Markdown snippet using backtick-delimited code spans.
///
/// The function scans the input line by line. Lines matching [`FENCE_RE`]
/// produce [`Token::Fence`] tokens and toggle fenced mode. Lines inside a
/// fence are yielded verbatim. Outside fenced regions the scanner searches for
/// backtick sequences. Text before a backtick becomes [`Token::Text`]. When a
/// closing backtick follows, the enclosed portion forms a [`Token::Code`]
/// span. If no closing backtick is found the delimiter and remaining text are
/// returned as [`Token::Text`]. Whitespace is preserved exactly as it appears.
///
/// ```rust
/// use crate::wrap::{Token, tokenize_markdown};
///
/// let tokens = tokenize_markdown("Example with `code`");
/// assert_eq!(
///     tokens,
///     vec![
///         Token::Text("Example with "),
///         Token::Code { raw: "`code`", fence: "`", code: "code" },
///     ]
/// );
/// ```
fn push_newline_if_needed<I>(
    tokens: &mut Vec<Token<'_>>,
    lines: &mut std::iter::Peekable<I>,
    had_trailing_newline: bool,
) where
    I: Iterator,
{
    // Emit a newline token if another line follows or when the
    // original input ended with a trailing newline. The peek avoids
    // prematurely allocating for the final newline when it isn't
    // necessary.
    if lines.peek().is_some() || (had_trailing_newline && lines.peek().is_none()) {
        tokens.push(Token::Newline);
    }
}

#[must_use]
pub fn tokenize_markdown(source: &str) -> Vec<Token<'_>> {
    if source.is_empty() {
        return Vec::new();
    }

    let mut tokens = Vec::new();
    let had_trailing_newline = source.ends_with('\n');
    let mut lines = source.lines().peekable();
    let mut in_fence = false;

    // Iterate lazily so we can safely use `peek()` to decide on trailing
    // newline emission without borrowing issues from a `for` loop over
    // `&str` references.
    while let Some(line) = lines.next() {
        if super::is_fence(line).is_some() {
            tokens.push(Token::Fence(line));
            push_newline_if_needed(&mut tokens, &mut lines, had_trailing_newline);
            in_fence = !in_fence;
            continue;
        }

        if in_fence {
            tokens.push(Token::Fence(line));
            push_newline_if_needed(&mut tokens, &mut lines, had_trailing_newline);
            continue;
        }

        tokenize_inline(line, &mut |tok| tokens.push(tok));
        push_newline_if_needed(&mut tokens, &mut lines, had_trailing_newline);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_inline_handles_multibyte_tokens() {
        let tokens = segment_inline("ßß `λ` фин");
        assert_eq!(
            tokens,
            vec![
                String::from("ßß"),
                String::from(" "),
                String::from("`λ`"),
                String::from(" "),
                String::from("фин"),
            ]
        );
    }

    #[test]
    fn link_with_trailing_punctuation() {
        let tokens = segment_inline("see [link](url).");
        assert_eq!(tokens, vec!["see", " ", "[link](url)", "."]);
    }

    #[test]
    fn image_with_nested_parentheses() {
        let tokens = segment_inline("![alt](path(a(b)c))");
        assert_eq!(tokens, vec!["![alt](path(a(b)c))"]);
    }

    #[test]
    fn inline_code_fences() {
        let tokens = segment_inline("use ``cmd`` now");
        assert_eq!(tokens, vec!["use", " ", "``cmd``", " ", "now"]);
    }

    #[test]
    fn unmatched_backticks() {
        let tokens = segment_inline("bad `code span");
        assert_eq!(tokens, vec!["bad", " ", "`", "code", " ", "span"]);
    }

    #[test]
    fn tokenize_marks_trailing_newline() {
        let tokens = tokenize_markdown("foo\n");
        assert_eq!(tokens, vec![Token::Text("foo"), Token::Newline]);
    }

    #[test]
    fn tokenize_handles_crlf() {
        let tokens = tokenize_markdown("foo\r\nbar");
        assert_eq!(
            tokens,
            vec![Token::Text("foo"), Token::Newline, Token::Text("bar")]
        );
    }

    #[test]
    fn escaped_triple_backticks_are_text() {
        let tokens = segment_inline(r"\`\`\`ignore");
        assert_eq!(tokens, vec![r"\`", r"\`", r"\`", "ignore"]);

        let tokens = tokenize_markdown(r"\`\`\`ignore");
        assert_eq!(tokens, vec![Token::Text(r"\`\`\`ignore")]);
    }

    #[test]
    fn escaped_inline_backtick_is_text() {
        let tokens = segment_inline(r"foo\`bar");
        assert_eq!(tokens, vec![r"foo\`", "bar"]);

        let tokens = tokenize_markdown(r"foo\`bar");
        assert_eq!(tokens, vec![Token::Text(r"foo\`bar")]);
    }

    #[test]
    fn escaped_backtick_adjacent_to_multibyte_characters_is_text() {
        let tokens = segment_inline(r"ß\`å");
        assert_eq!(tokens, vec![r"ß\`", "å"]);

        let tokens = tokenize_markdown(r"ß\`å");
        assert_eq!(tokens, vec![Token::Text(r"ß\`å")]);

        let tokens = segment_inline(r"前\`后");
        assert_eq!(tokens, vec![r"前\`", "后"]);

        let tokens = tokenize_markdown(r"前\`后");
        assert_eq!(tokens, vec![Token::Text(r"前\`后")]);
    }
}
