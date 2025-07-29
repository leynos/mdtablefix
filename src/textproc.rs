//! Token-level transformation utilities.
//!
//! This module provides helpers for processing Markdown input by
//! reusing the tokenizer from the [`wrap`] module. Each helper joins
//! incoming lines, tokenizes them, and feeds the tokens to caller
//! provided logic before splitting the output back into lines.

use crate::wrap::{Token, tokenize_markdown};

/// Apply a transformation to a sequence of [`Token`]s.
///
/// The `lines` slice is joined with newlines and tokenized. Each token
/// is passed to `f` along with the output accumulator. The final
/// string is split on newline characters and returned as a vector of
/// lines.
///
/// # Examples
///
/// ```
/// use mdtablefix::{
///     textproc::process_tokens,
///     wrap::{Token, tokenize_markdown},
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
    let joined = lines.join("\n");
    let mut out = String::new();
    for token in tokenize_markdown(&joined) {
        f(token, &mut out);
    }
    out.split('\n').map(str::to_string).collect()
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
}
