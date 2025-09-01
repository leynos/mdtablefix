//! Provides helpers for token-based transformations of Markdown lines.
//!
//! This module reuses the tokenizer from the [`crate::wrap`] module and offers
//! a streaming API for rewriting Markdown. Each helper tokenizes lines
//! on the fly, feeds the resulting tokens to caller-provided logic, and
//! then reconstructs the lines. Trailing blank lines roundtrip
//! correctly.

pub use crate::wrap::{Token, tokenize_markdown};

/// Append a [`Token`] to an output buffer without modification.
///
/// This helper reconstructs a token's original Markdown text. Callers can use
/// it to forward tokens they do not wish to transform while operating on text
/// tokens.
///
/// # Examples
///
/// ```rust
/// use mdtablefix::textproc::{Token, push_original_token};
///
/// let mut buf = String::new();
/// push_original_token(&Token::Code { fence: "`", code: "x" }, &mut buf);
/// assert_eq!(buf, "`x`");
/// ```
#[inline]
pub fn push_original_token(token: &Token<'_>, out: &mut String) {
    match token {
        Token::Text(t) => out.push_str(t),
        Token::Code { fence, code } => {
            out.push_str(fence);
            out.push_str(code);
            out.push_str(fence);
        }
        Token::Fence(f) => out.push_str(f),
        Token::Newline => out.push('\n'),
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
///     Token::Code { fence, code } => {
///         out.push_str(fence);
///        out.push_str(code);
///         out.push_str(fence);
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

    let source = lines.join("\n");
    let mut out = String::new();
    for token in tokenize_markdown(&source) {
        f(token, &mut out);
    }

    process_text(&out, trailing_blanks)
}

/// Split processed output into lines while preserving trailing blanks.
///
/// # Examples
///
/// ```rust
/// use mdtablefix::textproc::process_text;
///
/// let lines = process_text("a\nb\n", 0);
/// assert_eq!(lines, vec!["a".to_string(), "b".to_string(), String::new()]);
/// ```
#[must_use]
pub fn process_text(out: &str, trailing_blanks: usize) -> Vec<String> {
    if out.is_empty() {
        return Vec::new();
    }

    let had_trailing_newline = out.ends_with('\n');
    let mut result: Vec<String> = out.lines().map(ToOwned::to_owned).collect();
    if had_trailing_newline {
        result.push(String::new());
    }

    let out_blanks = result.iter().rev().take_while(|l| l.is_empty()).count();
    if out_blanks < trailing_blanks {
        result.extend(std::iter::repeat_n(
            String::new(),
            trailing_blanks - out_blanks,
        ));
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
            Token::Code { fence, code } => {
                buf.push_str(fence);
                buf.push_str(code);
                buf.push_str(fence);
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
    fn process_text_preserves_trailing_blank() {
        let lines = process_text("a\nb\n", 0);
        assert_eq!(lines, vec!["a".to_string(), "b".to_string(), String::new()]);
    }

    #[test]
    fn preserves_trailing_blank_lines() {
        let lines = vec!["a".to_string(), String::new(), String::new()];
        let out = process_tokens(&lines, |tok, buf| match tok {
            Token::Text(t) => buf.push_str(t),
            Token::Code { fence, code } => {
                buf.push_str(fence);
                buf.push_str(code);
                buf.push_str(fence);
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
            "Code { fence: \"``\", code: \"code\" }".to_string(),
            "Text(\" span\")".to_string(),
        ];
        assert_eq!(tokens, expected);
    }

    #[test]
    fn push_original_token_roundtrips_all_variants() {
        let mut buf = String::new();

        push_original_token(&Token::Text("a"), &mut buf);
        assert_eq!(buf, "a");

        buf.clear();
        push_original_token(
            &Token::Code {
                fence: "`",
                code: "b",
            },
            &mut buf,
        );
        assert_eq!(buf, "`b`");

        buf.clear();
        push_original_token(&Token::Fence("```"), &mut buf);
        assert_eq!(buf, "```");

        buf.clear();
        push_original_token(&Token::Newline, &mut buf);
        assert_eq!(buf, "\n");
    }
}
