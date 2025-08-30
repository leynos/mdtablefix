//! Fix misplaced emphasis around inline code spans.
//!
//! Emphasis markers that directly adjoin backtick-wrapped inline code without
//! spaces are reordered or stripped so code remains intact within emphasised
//! text.

use crate::textproc::process_text;
use crate::wrap::{Token, tokenize_markdown};

/// Split emphasis markers at both ends of `s`.
///
/// Returns a triple of leading markers, core text and trailing markers.
///
/// # Examples
///
/// ```
/// assert_eq!(split_marks("**bold**"), ("**", "bold", "**"));
/// assert_eq!(split_marks("text"), ("", "text", ""));
/// ```
fn split_marks(s: &str) -> (&str, &str, &str) {
    let start = s.find(|c| c != '*' && c != '_').unwrap_or(0);
    let end = s.rfind(|c| c != '*' && c != '_').map_or(s.len(), |i| i + 1);
    (&s[..start], &s[start..end], &s[end..])
}

fn push_code(code: &str, out: &mut String) {
    out.push('`');
    out.push_str(code);
    out.push('`');
}

/// Merge contiguous code and emphasis spans.
///
/// Groups of emphasis markers and inline code with no separating spaces are
/// normalised so that emphasis markers wrap the entire group or are removed
/// when they solely surround code.
///
/// # Examples
///
/// ```
/// use mdtablefix::code_emphasis::fix_code_emphasis;
/// let lines = vec!["`code`**text**".to_string()];
/// assert_eq!(
///     fix_code_emphasis(&lines),
///     vec!["**`code`text**".to_string()]
/// );
/// ```
#[must_use]
pub fn fix_code_emphasis(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }
    let trailing_blanks = lines.iter().rev().take_while(|l| l.is_empty()).count();
    if trailing_blanks == lines.len() {
        return vec![String::new(); lines.len()];
    }
    let source = lines.join("\n");
    let mut tokens = tokenize_markdown(&source).into_iter().peekable();
    let mut out = String::new();
    let mut pending = "";
    while let Some(token) = tokens.next() {
        match token {
            Token::Text(raw) => {
                if tokens.peek().is_some_and(|t| matches!(t, Token::Code(_))) {
                    let (lead, body, trail) = split_marks(raw);
                    out.push_str(lead);
                    out.push_str(body);
                    pending = trail;
                } else {
                    out.push_str(raw);
                }
            }
            Token::Code(code) => {
                let mut prefix = pending;
                pending = "";
                if let Some(Token::Text(next)) = tokens.peek_mut() {
                    let (lead, _, _) = split_marks(next);
                    if !lead.is_empty() {
                        prefix = if prefix.is_empty() { lead } else { "" };
                        *next = &next[lead.len()..];
                    }
                }
                if !prefix.is_empty() {
                    out.push_str(prefix);
                }
                push_code(code, &mut out);
            }
            Token::Fence(f) => out.push_str(f),
            Token::Newline => out.push('\n'),
        }
    }
    process_text(&out, trailing_blanks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_emphasis_and_code() {
        let input = vec![
            "`StepContext`** Enhancement (in **`crates/rstest-bdd/src/context.rs`**)**".to_string(),
        ];
        let expected = vec![
            "**`StepContext` Enhancement (in `crates/rstest-bdd/src/context.rs`)**".to_string(),
        ];
        assert_eq!(fix_code_emphasis(&input), expected);
    }

    #[test]
    fn ignores_simple_text() {
        let input = vec!["nothing here".to_string()];
        assert_eq!(fix_code_emphasis(&input), input);
    }
}
