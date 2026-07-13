//! Tokenization helpers for wrapping logic.
//!
//! This module contains utilities for breaking lines into tokens so that
//! inline code spans and Markdown links are preserved during wrapping.
//! Full-line fenced code blocks are tracked by [`tokenize_markdown`] with the
//! shared [`FenceTracker`] from `super::fence`; `fence.rs` owns that tracker
//! and its opening, closing, marker-length, and nested-literal semantics. When
//! [`FenceTracker::observe`] reports a fence boundary, that line is emitted as
//! [`Token::Fence`]. Subsequent lines inside the open fence are also emitted as
//! [`Token::Fence`], preserving their byte content verbatim until the matching
//! closing fence marker is seen. This prevents `--ellipsis`, `--wrap`, and
//! similar post-processors from mutating fenced code block contents, fixing
//! issue `#329`.

mod link_span;
mod parsing;
mod scanning;

pub(crate) use link_span::link_or_image_span;
use parsing::{
    handle_backtick_fence,
    is_trailing_punctuation,
    looks_like_image_start,
    parse_link_or_image,
};
#[cfg(test)]
pub(crate) use scanning::continuation_begins_with_closing_fence;
#[cfg(test)]
pub(crate) use scanning::has_unclosed_code_span;
use scanning::{bracket_follows_escaped_bang, collect_range, scan_code_suffix_end, scan_while};
pub(crate) use scanning::{
    has_odd_backslash_escape_bytes,
    opening_fence_run_len,
    parse_open_code_span,
    position_after_close,
    scan_continuation_span_state,
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
        }

        if ch == '`' {
            if has_odd_backslash_escape_bytes(bytes, i) {
                append_escaped_backtick(&mut tokens);
                i += ch.len_utf8();
                continue;
            }

            let (token, new_i) = handle_backtick_fence(text, i);
            let (token, new_i) = extend_closed_code_token(text, i, token, new_i);
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
            new_i = scan_trailing_punctuation_end(text, new_i);
            if new_i > punct_start {
                tokens.push(collect_range(text, punct_start, new_i));
            }
            i = new_i;
            continue;
        }

        let start = i;
        i = scan_plain_text_end(text, bytes, i);
        tokens.push(collect_range(text, start, i));
    }
    tokens
}

fn scan_trailing_punctuation_end(text: &str, mut index: usize) -> usize {
    while index < text.len() {
        let Some(current) = text[index..].chars().next() else {
            break;
        };
        if starts_inline_citation(text, index) {
            break;
        }
        if !is_trailing_punctuation(current) {
            break;
        }
        index += current.len_utf8();
    }
    index
}

fn starts_inline_citation(text: &str, index: usize) -> bool {
    text.get(index..).is_some_and(|tail| tail.starts_with("(["))
}

fn append_escaped_backtick(tokens: &mut Vec<String>) {
    if let Some(last) = tokens.last_mut() {
        last.push('`');
    } else {
        tokens.push(String::from("`"));
    }
}

fn scan_plain_text_end(text: &str, bytes: &[u8], mut index: usize) -> usize {
    if starts_inline_citation(text, index) && !has_odd_backslash_escape_bytes(bytes, index) {
        return index + 1;
    }

    while index < text.len() {
        let Some(current) = text[index..].chars().next() else {
            break;
        };
        if current.is_whitespace() || current == '`' {
            break;
        }

        let current_escaped = has_odd_backslash_escape_bytes(bytes, index);
        if should_stop_plain_text(text, bytes, index, (current, current_escaped)) {
            break;
        }

        index += current.len_utf8();
    }
    index
}

fn should_stop_plain_text(text: &str, bytes: &[u8], index: usize, current: (char, bool)) -> bool {
    let (ch, is_escaped) = current;
    if ch == '[' {
        return !is_escaped
            && !bracket_follows_escaped_bang(bytes, index)
            && !bracket_follows_escaped_open_paren(bytes, index);
    }
    if ch == '(' {
        return !is_escaped && starts_inline_citation(text, index);
    }
    looks_like_image_start(text, index, ch) && !is_escaped
}

fn bracket_follows_escaped_open_paren(bytes: &[u8], index: usize) -> bool {
    index.checked_sub(1).is_some_and(|previous| {
        bytes[previous] == b'(' && has_odd_backslash_escape_bytes(bytes, previous)
    })
}

fn is_closed_inline_code_span(token: &str) -> bool {
    let fence_len = token.chars().take_while(|&ch| ch == '`').count();
    fence_len > 0 && token.len() > fence_len * 2 && token.ends_with(&"`".repeat(fence_len))
}

fn extend_closed_code_token(
    text: &str,
    start: usize,
    token: String,
    code_end: usize,
) -> (String, usize) {
    if !is_closed_inline_code_span(&token) {
        return (token, code_end);
    }
    let suffix_end = scan_code_suffix_end(text, code_end);
    if suffix_end > code_end {
        (collect_range(text, start, suffix_end), suffix_end)
    } else {
        (token, code_end)
    }
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

    // SAFETY: backtick (U+0060) is a one-byte ASCII codepoint, so the
    // character count from `take_while` equals the byte length of the
    // fence delimiter. Slicing by `delim_len` is a valid UTF-8 boundary.
    let fence = &rest[..delim_len];
    if let Some(raw_end) = position_after_close(rest, delim_len, delim_len) {
        let candidate = raw_end - delim_len;
        let token = &rest[..raw_end];
        let suffix_end = if is_closed_inline_code_span(token) {
            scan_code_suffix_end(rest, raw_end)
        } else {
            raw_end
        };
        let code = &rest[delim_len..candidate];
        let raw = &rest[..suffix_end];
        return Some((Token::Code { raw, fence, code }, suffix_end));
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
/// Tokenizes inline Markdown `source` into an ordered list of [`Token`] values.
///
/// The `source` parameter is the inline Markdown text to tokenize. The return
/// value is `Vec<Token<'_>>`, preserving the token order found in `source`.
/// Inline code spans, links, and whitespace runs are emitted as distinct token
/// variants or text slices so callers can perform width-aware or
/// format-aware processing.
///
/// ```rust
/// use mdtablefix::wrap::{Token, tokenize_markdown};
///
/// let tokens = tokenize_markdown("Example with `code`");
/// assert_eq!(
///     tokens,
///     vec![
///         Token::Text("Example with "),
///         Token::Code {
///             raw: "`code`",
///             fence: "`",
///             code: "code"
///         },
///     ]
/// );
/// ```
pub fn tokenize_markdown(source: &str) -> Vec<Token<'_>> {
    if source.is_empty() {
        return Vec::new();
    }

    let mut tokens = Vec::new();
    let had_trailing_newline = source.ends_with('\n');
    let mut lines = source.lines().peekable();
    let mut fence_tracker = super::FenceTracker::default();

    // Iterate lazily so we can safely use `peek()` to decide on trailing
    // newline emission without borrowing issues from a `for` loop over
    // `&str` references.
    while let Some(line) = lines.next() {
        if fence_tracker.observe(line, 0) {
            tokens.push(Token::Fence(line));
            push_newline_if_needed(&mut tokens, &mut lines, had_trailing_newline);
            continue;
        }

        if fence_tracker.in_fence(0) {
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
#[path = "mod_tests.rs"]
mod tests;
