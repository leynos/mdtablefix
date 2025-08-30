//! Fix misplaced emphasis around inline code spans.
//!
//! Emphasis markers that directly adjoin backtick-wrapped inline code without
//! spaces are reordered or stripped so code remains intact within emphasised
//! text.

use crate::textproc::process_text;
use crate::wrap::{Token, tokenize_markdown};

fn split_leading_emphasis(s: &str) -> (&str, &str) {
    let idx = s.find(|c| c != '*' && c != '_').unwrap_or(s.len());
    s.split_at(idx)
}

fn split_trailing_emphasis(s: &str) -> (&str, &str) {
    let idx = s.rfind(|c| c != '*' && c != '_').map_or(0, |i| i + 1);
    s.split_at(idx)
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
    let tokens = tokenize_markdown(&source);
    let mut out = String::new();
    let mut pending_prefix: Option<String> = None;
    let mut next_override: Option<String> = None;
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Text(t_raw) => {
                let t = next_override.take().unwrap_or_else(|| (*t_raw).to_string());
                if matches!(tokens.get(i + 1), Some(Token::Code(_))) {
                    let (body, emph) = split_trailing_emphasis(&t);
                    out.push_str(body);
                    if !emph.is_empty() {
                        pending_prefix = Some(emph.to_string());
                    }
                } else {
                    out.push_str(&t);
                }
                i += 1;
            }
            Token::Code(c) => {
                let mut prefix = pending_prefix.take().unwrap_or_default();
                if let Some(Token::Text(t_next)) = tokens.get(i + 1) {
                    let (emph, rest) = split_leading_emphasis(t_next);
                    if !emph.is_empty() {
                        if prefix.is_empty() {
                            prefix = emph.to_string();
                        } else {
                            prefix.clear();
                        }
                        next_override = Some(rest.to_string());
                    }
                }
                if !prefix.is_empty() {
                    out.push_str(&prefix);
                }
                push_code(c, &mut out);
                i += 1;
            }
            Token::Fence(f) => {
                out.push_str(f);
                i += 1;
            }
            Token::Newline => {
                out.push('\n');
                i += 1;
            }
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
