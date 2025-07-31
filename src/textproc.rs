//! Provides helpers for token-based transformations of Markdown lines.
//!
//! This module reuses the tokenizer from the [`wrap`] module and offers
//! a streaming API for rewriting Markdown. Each helper tokenizes lines
//! on the fly, feeds the resulting tokens to caller-provided logic, and
//! then reconstructs the lines. Trailing blank lines roundtrip
//! correctly.

pub use crate::wrap::Token;
use crate::wrap::is_fence;

fn tokenize_inline<'a, F>(text: &'a str, emit: &mut F)
where
    F: FnMut(Token<'a>),
{
    let mut rest = text;
    while let Some(pos) = rest.find('`') {
        if pos > 0 {
            emit(Token::Text(&rest[..pos]));
        }
        let delim_len = rest[pos..].chars().take_while(|&c| c == '`').count();
        let search = &rest[pos + delim_len..];
        let closing = "`".repeat(delim_len);
        if let Some(end) = search.find(&closing) {
            emit(Token::Code(&rest[pos + delim_len..pos + delim_len + end]));
            rest = &search[end + delim_len..];
        } else {
            emit(Token::Text(&rest[pos..]));
            rest = "";
            break;
        }
    }
    if !rest.is_empty() {
        emit(Token::Text(rest));
    }
}

fn handle_line<'a, F>(line: &'a str, last: bool, in_fence: &mut bool, f: &mut F, out: &mut String)
where
    F: FnMut(Token<'a>, &mut String),
{
    if is_fence(line) {
        f(Token::Fence(line), out);
        if !last {
            f(Token::Newline, out);
        }
        *in_fence = !*in_fence;
        return;
    }

    if *in_fence {
        f(Token::Fence(line), out);
        if !last {
            f(Token::Newline, out);
        }
        return;
    }

    tokenize_inline(line, &mut |tok| f(tok, out));
    if !last {
        f(Token::Newline, out);
    }
}

/// Apply a transformation to a sequence of [`Token`]s.
///
/// The `lines` slice is tokenized in order, preserving fence context.
/// Each token is passed to `f` along with the output accumulator. The
/// final string is split on newline characters and returned as a
/// vector of lines.
///
/// # Examples
///
/// ```rust
/// use mdtablefix::{textproc::process_tokens, wrap::Token};
///
/// let lines = vec!["code".to_string()];
/// let out = process_tokens(&lines, |tok, out| match tok {
///     Token::Text(t) => out.push_str(t),
///     Token::Code(c) => {
///         out.push('`');
///         out.push_str(c);
///         out.push('`');
///     }
///     Token::Fence(f) => out.push_str(f),
///     Token::Newline => out.push('\n'),
/// });
/// assert_eq!(out, lines);
/// ```
#[must_use]
pub fn process_tokens<F>(lines: &[String], mut f: F) -> Vec<String>
where
    F: FnMut(Token<'_>, &mut String),
{
    if lines.is_empty() {
        return Vec::new();
    }

    let trailing_blanks = lines.iter().rev().take_while(|l| l.is_empty()).count();
    if trailing_blanks == lines.len() {
        return vec![String::new(); lines.len()];
    }

    let mut out = String::new();
    let mut in_fence = false;
    let last_idx = lines.len() - 1;
    for (i, line) in lines.iter().enumerate() {
        handle_line(line, i == last_idx, &mut in_fence, &mut f, &mut out);
    }

    if out.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<String> = out.split('\n').map(str::to_string).collect();
    let out_blanks = result.iter().rev().take_while(|l| l.is_empty()).count();
    for _ in out_blanks..trailing_blanks {
        result.push(String::new());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_transformation_returns_input() {
        let lines = vec!["a `b`".to_string()];
        let out = process_tokens(&lines, |tok, buf| match tok {
            Token::Text(t) => buf.push_str(t),
            Token::Code(c) => {
                buf.push('`');
                buf.push_str(c);
                buf.push('`');
            }
            Token::Fence(f) => buf.push_str(f),
            Token::Newline => buf.push('\n'),
        });
        assert_eq!(out, lines);
    }

    #[test]
    fn empty_input_returns_empty_vector() {
        let lines: Vec<String> = Vec::new();
        let out = process_tokens(&lines, |_tok, _out| unreachable!());
        assert!(out.is_empty());
    }

    #[test]
    fn transformation_can_remove_all_content() {
        let lines = vec!["data".to_string()];
        let out = process_tokens(&lines, |_tok, _out| {});
        assert!(out.is_empty());
    }

    #[test]
    fn preserves_trailing_blank_lines() {
        let lines = vec!["a".to_string(), String::new(), String::new()];
        let out = process_tokens(&lines, |tok, buf| match tok {
            Token::Text(t) => buf.push_str(t),
            Token::Code(c) => {
                buf.push('`');
                buf.push_str(c);
                buf.push('`');
            }
            Token::Fence(f) => buf.push_str(f),
            Token::Newline => buf.push('\n'),
        });
        assert_eq!(out, lines);
    }

    #[test]
    fn blanks_only_are_preserved() {
        let lines = vec![String::new(), String::new()];
        let out = process_tokens(&lines, |_tok, _buf| {});
        assert_eq!(out, lines);
    }

    #[test]
    fn token_stream_handles_fences() {
        let lines = vec![
            "```rust".to_string(),
            "fn main() {".to_string(),
            "    println!(\"hi\");".to_string(),
            "```".to_string(),
        ];
        let mut tokens = Vec::new();
        let _ = process_tokens(&lines, |tok, _| tokens.push(format!("{tok:?}")));
        let expected = vec![
            "Fence(\"```rust\")".to_string(),
            "Newline".to_string(),
            "Fence(\"fn main() {\")".to_string(),
            "Newline".to_string(),
            "Fence(\"    println!(\\\"hi\\\");\")".to_string(),
            "Newline".to_string(),
            "Fence(\"```\")".to_string(),
        ];
        assert_eq!(tokens, expected);
    }

    #[test]
    fn malformed_fence_sequence_returns_tokens() {
        let lines = vec!["```".to_string(), "code".to_string()];
        let mut tokens = Vec::new();
        let _ = process_tokens(&lines, |tok, _| tokens.push(format!("{tok:?}")));
        let expected = vec![
            "Fence(\"```\")".to_string(),
            "Newline".to_string(),
            "Fence(\"code\")".to_string(),
        ];
        assert_eq!(tokens, expected);
    }

    #[test]
    fn multi_backtick_spans_are_recognised() {
        let lines = vec!["A ``code`` span".to_string()];
        let mut tokens = Vec::new();
        let _ = process_tokens(&lines, |tok, _| tokens.push(format!("{tok:?}")));
        let expected = vec![
            "Text(\"A \")".to_string(),
            "Code(\"code\")".to_string(),
            "Text(\" span\")".to_string(),
        ];
        assert_eq!(tokens, expected);
    }
}
