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

fn parse_link_or_image(chars: &[char], mut i: usize) -> (String, usize) {
    let start = i;
    if chars[i] == '!' {
        i += 1;
    }
    // skip initial '[' which we know is present
    i += 1;
    while i < chars.len() && chars[i] != ']' {
        i += 1;
    }
    if i < chars.len() && chars[i] == ']' {
        i += 1;
        if i < chars.len() && chars[i] == '(' {
            i += 1;
            let mut depth = 1;
            while i < chars.len() && depth > 0 {
                match chars[i] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            let tok: String = chars[start..i].iter().collect();
            return (tok, i);
        }
    }
    let tok: String = chars[start..=start].iter().collect();
    (tok, start + 1)
}

pub(super) fn tokenize_inline(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            let start = i;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
        } else if c == '`' {
            let start = i;
            let mut delim_len = 0;
            while i < chars.len() && chars[i] == '`' {
                i += 1;
                delim_len += 1;
            }
            let mut end = i;
            while end < chars.len() {
                if chars[end] == '`' {
                    let mut j = end;
                    let mut count = 0;
                    while j < chars.len() && chars[j] == '`' {
                        j += 1;
                        count += 1;
                    }
                    if count == delim_len {
                        end = j;
                        break;
                    }
                }
                end += 1;
            }
            if end >= chars.len() {
                tokens.push(chars[start..start + delim_len].iter().collect());
                i = start + delim_len;
            } else {
                tokens.push(chars[start..end].iter().collect());
                i = end;
            }
        } else if c == '[' || (c == '!' && i + 1 < chars.len() && chars[i + 1] == '[') {
            let (tok, new_i) = parse_link_or_image(&chars, i);
            tokens.push(tok);
            i = new_i;
        } else {
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '`' {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
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
