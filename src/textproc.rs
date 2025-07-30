//! Provides helpers for token-based transformations of Markdown lines.
//!
//! This module reuses the tokenizer from the [`wrap`] module and offers
//! a streaming API for rewriting Markdown. Each helper tokenizes lines
//! on the fly, feeds the resulting tokens to caller-provided logic, and
//! then reconstructs the lines. Trailing blank lines roundtrip
//! correctly.

use crate::wrap::{Token, is_fence};

/// Apply a transformation to a sequence of [`Token`]s.
///
/// The `lines` slice is tokenized in order, preserving fence context.
/// Each token is passed to `f` along with the output accumulator. The
/// final string is split on newline characters and returned as a
/// vector of lines.
///
/// # Examples
///
/// ```ignore
/// use mdtablefix::{
///     textproc::process_tokens,
///     wrap::Token,
/// };
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
pub(crate) fn process_tokens<F>(lines: &[String], mut f: F) -> Vec<String>
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
        let trimmed = line.as_str();
        if is_fence(trimmed) {
            f(Token::Fence(trimmed), &mut out);
            if i < last_idx {
                f(Token::Newline, &mut out);
            }
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            f(Token::Fence(trimmed), &mut out);
            if i < last_idx {
                f(Token::Newline, &mut out);
            }
            continue;
        }
        let mut rest = trimmed;
        while let Some(pos) = rest.find('`') {
            if pos > 0 {
                f(Token::Text(&rest[..pos]), &mut out);
            }
            if let Some(end) = rest[pos + 1..].find('`') {
                f(Token::Code(&rest[pos + 1..pos + 1 + end]), &mut out);
                rest = &rest[pos + end + 2..];
            } else {
                f(Token::Text(&rest[pos..]), &mut out);
                rest = "";
                break;
            }
        }
        if !rest.is_empty() {
            f(Token::Text(rest), &mut out);
        }
        if i < last_idx {
            f(Token::Newline, &mut out);
        }
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
}
