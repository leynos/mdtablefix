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

mod inline_tokens;

mod link_span;

mod parsing;

mod scanning;

use inline_tokens::is_closed_inline_code_span;
#[cfg(test)]
pub(in crate::wrap) use inline_tokens::segment_inline;
pub(in crate::wrap) use inline_tokens::segment_inline_observed;
pub(crate) use link_span::link_or_image_span;
#[cfg(test)]
pub(crate) use scanning::continuation_begins_with_closing_fence;
#[cfg(test)]
pub(crate) use scanning::has_unclosed_code_span;
use scanning::scan_code_suffix_end;
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
        /// The span as written: its fences, the code, and any inflectional suffix
        /// attached to the closing fence (`` `Name`s ``).
        raw: &'a str,
        /// The backtick fence that opens and closes the span.
        fence: &'a str,
        /// The text between the fences.
        code: &'a str,
    },
    /// Plain text outside code regions.
    Text(&'a str),
    /// Line break separating tokens.
    Newline,
}

/// Produce one token and its consumed byte count from an inline source offset.
///
/// Every returned offset is a UTF-8 boundary; the scanner either emits a whole
/// completed code span or advances through literal text to prevent stalling.
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

/// Preserve source line boundaries that survive tokenization.
///
/// A newline token is retained between lines and after a trailing newline so
/// wrapping can distinguish paragraph separation from an absent final break.
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
        let fence = fence_tracker.observe_source_line(line);
        if fence.is_fence_marker || fence.is_in_fence {
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
