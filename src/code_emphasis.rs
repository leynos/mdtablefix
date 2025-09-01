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
    let first = s.find(|c| c != '*' && c != '_').unwrap_or(s.len());
    let last = s.rfind(|c| c != '*' && c != '_').map_or(first, |i| i + 1);
    (&s[..first], &s[first..last], &s[last..])
}

fn push_code(code: &str, out: &mut String) {
    let mut max_run = 0;
    let mut run = 0;
    for c in code.chars() {
        if c == '`' {
            run += 1;
            max_run = max_run.max(run);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat(max_run + 1);
    out.push_str(&fence);
    out.push_str(code);
    out.push_str(&fence);
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
                    if body.is_empty() && trail.is_empty() {
                        pending = lead;
                    } else {
                        out.push_str(lead);
                        out.push_str(body);
                        pending = trail;
                    }
                } else {
                    out.push_str(raw);
                }
            }
            Token::Code(code) => {
                let mut prefix = pending;
                let mut suffix = "";
                pending = "";
                if let Some(Token::Text(next)) = tokens.peek_mut() {
                    let (lead, mid, _) = split_marks(next);
                    if !lead.is_empty() {
                        if prefix.is_empty() {
                            prefix = lead;
                        } else if mid.is_empty() {
                            suffix = lead;
                        } else {
                            prefix = "";
                        }
                        *next = &next[lead.len()..];
                    }
                }
                if !prefix.is_empty() {
                    out.push_str(prefix);
                }
                push_code(code, &mut out);
                if !suffix.is_empty() {
                    out.push_str(suffix);
                }
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

    #[test]
    fn preserves_emphasised_code_only() {
        let input = vec!["**`code`**".to_string()];
        assert_eq!(fix_code_emphasis(&input), input);
    }

    #[test]
    fn preserves_inner_backticks_in_code() {
        let input = vec!["``a`b``".to_string()];
        assert_eq!(fix_code_emphasis(&input), input);
    }
}
