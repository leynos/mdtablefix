//! Inline tokenization helpers for wrapping logic.
//!
//! This module exposes [`Token`] and parsing utilities used by the
//! higher-level wrapping functions.
/// Markdown token emitted by [`tokenize_markdown`].
use super::FENCE_RE;
#[derive(Debug, PartialEq)]
pub enum Token<'a> {
    /// Line within a fenced code block, including the fence itself.
    Fence(&'a str),
    /// Inline code span without surrounding backticks.
    Code(&'a str),
    /// Plain text outside code regions.
    Text(&'a str),
    /// Line break separating tokens.
    Newline,
}

fn scan_while<F>(s: &str, mut pos: usize, mut pred: F) -> usize
where
    F: FnMut(char) -> bool,
{
    while let Some(ch) = s[pos..].chars().next() {
        if !pred(ch) {
            break;
        }
        pos += ch.len_utf8();
    }
    pos
}

fn parse_link_or_image(text: &str, start: usize) -> usize {
    let mut pos = start;
    if text.as_bytes()[pos] == b'!' {
        pos += 1;
    }
    pos += 1; // skip '['
    if let Some(end_br) = text[pos..].find("](") {
        let mut i = pos + end_br + 2;
        let mut depth = 1;
        while i < text.len() && depth > 0 {
            let ch = text[i..].chars().next().expect("valid UTF-8");
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            i += ch.len_utf8();
        }
        if depth == 0 {
            return i;
        }
    }
    start + text[start..].chars().next().unwrap().len_utf8()
}

pub(crate) fn tokenize_inline(text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut pos = 0;
    while pos < text.len() {
        let ch = text[pos..].chars().next().expect("valid UTF-8");
        if ch.is_whitespace() {
            let start = pos;
            pos = scan_while(text, start, char::is_whitespace);
            tokens.push(&text[start..pos]);
        } else if ch == '`' {
            let start = pos;
            pos = scan_while(text, start, |c| c == '`');
            let delim = &text[start..pos];
            let mut end = pos;
            let mut found = false;
            while end < text.len() {
                if text[end..].starts_with(delim) {
                    end += delim.len();
                    tokens.push(&text[start..end]);
                    pos = end;
                    found = true;
                    break;
                }
                end += text[end..].chars().next().unwrap().len_utf8();
            }
            if !found {
                tokens.push(delim);
                pos = start + delim.len();
            }
        } else if ch == '[' || (ch == '!' && text[pos + ch.len_utf8()..].starts_with('[')) {
            let end = parse_link_or_image(text, pos);
            tokens.push(&text[pos..end]);
            pos = end;
        } else {
            let start = pos;
            pos = scan_while(text, start, |c| !c.is_whitespace() && c != '`' && c != '[');
            tokens.push(&text[start..pos]);
        }
    }
    tokens
}

/// Split the input string into [`Token`]s by analysing whitespace and
/// backtick delimiters.
///
/// The tokenizer groups consecutive whitespace into a single
/// [`Token::Text`] and recognises backtick sequences as inline code spans.
/// When a run of backticks is encountered the parser searches forward for an
/// identical delimiter, allowing nested backticks when the span uses a longer
/// fence. Unmatched delimiter sequences are treated as literal text.
///
/// ```rust,ignore
/// use mdtablefix::wrap::{Token, tokenize_markdown};
///
/// let tokens = tokenize_markdown("Example with `code`");
/// assert_eq!(
///     tokens,
///     vec![Token::Text("Example with "), Token::Code("code")]
/// );
/// ```
pub(crate) fn tokenize_markdown(input: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in input.split_inclusive('\n') {
        let trimmed = line.trim_end_matches('\n');
        if FENCE_RE.is_match(trimmed) {
            out.push(Token::Fence(trimmed));
            out.push(Token::Newline);
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            out.push(Token::Fence(trimmed));
            out.push(Token::Newline);
            continue;
        }
        let mut rest = trimmed;
        while let Some(pos) = rest.find('`') {
            if pos > 0 {
                out.push(Token::Text(&rest[..pos]));
            }
            if let Some(end) = rest[pos + 1..].find('`') {
                out.push(Token::Code(&rest[pos + 1..pos + 1 + end]));
                rest = &rest[pos + end + 2..];
            } else {
                out.push(Token::Text(&rest[pos..]));
                rest = "";
                break;
            }
        }
        if !rest.is_empty() {
            out.push(Token::Text(rest));
        }
        out.push(Token::Newline);
    }
    out.pop();
    out
}
