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
/// push_original_token(
///     &Token::Code {
///         raw: "`x`",
///         fence: "`",
///         code: "x",
///     },
///     &mut buf,
/// );
/// assert_eq!(buf, "`x`");
/// ```
#[inline]
pub fn push_original_token(token: &Token<'_>, out: &mut String) {
    match token {
        Token::Text(t) => out.push_str(t),
        Token::Code { raw, .. } => out.push_str(raw),
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
///     Token::Code { raw, .. } => out.push_str(raw),
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
    let mut out = String::with_capacity(source.len());
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

/// Return the leading Unicode-whitespace prefix of `s` without allocating.
///
/// Whitespace is defined by [`char::is_whitespace`]. The returned slice is
/// empty when `s` begins with a non-whitespace character and is `s` itself
/// when every character is whitespace.
///
/// # Examples
///
/// ```
/// use mdtablefix::textproc::leading_indent;
///
/// assert_eq!(leading_indent("  hello"), "  ");
/// ```
#[inline]
#[must_use]
pub fn leading_indent(s: &str) -> &str {
    let end = s
        .char_indices()
        .find_map(|(index, character)| (!character.is_whitespace()).then_some(index))
        .unwrap_or(s.len());
    s.get(..end).unwrap_or(s)
}

#[cfg(test)]
mod tests {
    //! Unit tests for token processing.

    use super::*;

    #[test]
    fn identity_transformation_returns_input() {
        let lines = vec!["a `b`".to_owned()];
        let out = process_tokens(&lines, |tok, buf| match tok {
            Token::Text(t) => buf.push_str(t),
            Token::Code { raw, .. } => buf.push_str(raw),
            Token::Fence(f) => buf.push_str(f),
            Token::Newline => buf.push('\n'),
        });
        assert_eq!(out, lines);
    }

    #[test]
    fn empty_input_returns_empty_vector() {
        let lines: Vec<String> = Vec::new();
        let mut was_called = false;
        let out = process_tokens(&lines, |_tok, _out| was_called = true);
        assert!(out.is_empty());
        assert!(!was_called, "empty input must not invoke the callback");
    }

    #[test]
    fn transformation_can_remove_all_content() {
        let lines = vec!["data".to_owned()];
        let out = process_tokens(&lines, |_tok, _out| {});
        assert!(out.is_empty());
    }

    #[test]
    fn process_text_preserves_trailing_blank() {
        let lines = process_text("a\nb\n", 0);
        assert_eq!(lines, vec!["a".to_owned(), "b".to_owned(), String::new()]);
    }

    #[test]
    fn preserves_trailing_blank_lines() {
        let lines = vec!["a".to_owned(), String::new(), String::new()];
        let out = process_tokens(&lines, |tok, buf| match tok {
            Token::Text(t) => buf.push_str(t),
            Token::Code { raw, .. } => buf.push_str(raw),
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
            "```rust".to_owned(),
            "fn main() {".to_owned(),
            "    println!(\"hi\");".to_owned(),
            "```".to_owned(),
        ];
        let mut tokens = Vec::new();
        let processed = process_tokens(&lines, |tok, _| tokens.push(format!("{tok:?}")));
        assert!(
            processed.is_empty(),
            "token capture must leave output empty"
        );
        let expected = vec![
            "Fence(\"```rust\")".to_owned(),
            "Newline".to_owned(),
            "Fence(\"fn main() {\")".to_owned(),
            "Newline".to_owned(),
            "Fence(\"    println!(\\\"hi\\\");\")".to_owned(),
            "Newline".to_owned(),
            "Fence(\"```\")".to_owned(),
        ];
        assert_eq!(tokens, expected);
    }

    #[test]
    fn malformed_fence_sequence_returns_tokens() {
        let lines = vec!["```".to_owned(), "code".to_owned()];
        let mut tokens = Vec::new();
        let processed = process_tokens(&lines, |tok, _| tokens.push(format!("{tok:?}")));
        assert!(
            processed.is_empty(),
            "token capture must leave output empty"
        );
        let expected = vec![
            "Fence(\"```\")".to_owned(),
            "Newline".to_owned(),
            "Fence(\"code\")".to_owned(),
        ];
        assert_eq!(tokens, expected);
    }

    #[test]
    fn multi_backtick_spans_are_recognised() {
        let lines = vec!["A ``code`` span".to_owned()];
        let mut tokens = Vec::new();
        let processed = process_tokens(&lines, |tok, _| tokens.push(format!("{tok:?}")));
        assert!(
            processed.is_empty(),
            "token capture must leave output empty"
        );
        let expected = vec![
            "Text(\"A \")".to_owned(),
            "Code { raw: \"``code``\", fence: \"``\", code: \"code\" }".to_owned(),
            "Text(\" span\")".to_owned(),
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
                raw: "`b`",
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
