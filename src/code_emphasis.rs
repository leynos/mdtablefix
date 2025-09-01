//! Fix misplaced emphasis around inline code spans.
//!
//! The pass normalises emphasis markers that directly adjoin
//! backtick-wrapped inline code. Only `*` and `_` markers are considered; other
//! flavours such as tildes are ignored. Inline code is re-serialised using a
//! backtick fence long enough to contain any inner backticks without escaping.
//! Spans without adjacent emphasis markers are returned verbatim.
//!
//! Mixed surrounding markers (for example `*code**`) are left untouched. This
//! transformation should run before wrapping and footnote conversion so marker
//! adjacency is evaluated on the raw input.

use crate::textproc::process_text;
use crate::wrap::{Token, tokenize_markdown};

/// Split emphasis markers at both ends of `s`.
///
/// Returns a triple of leading markers, core text and trailing markers.
///
/// # Examples
///
/// ```ignore
/// // Internal helper; see unit tests for coverage.
/// // assert_eq!(split_marks("**bold**"), ("**", "bold", "**"));
/// // assert_eq!(split_marks("text"), ("", "text", ""));
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
    if !source.contains("`*")
        && !source.contains("`_")
        && !source.contains("*`")
        && !source.contains("_`")
    {
        return lines.to_vec();
    }
    let mut tokens = tokenize_markdown(&source).into_iter().peekable();
    let mut out = String::new();
    let mut pending = "";
    while let Some(token) = tokens.next() {
        match token {
            Token::Text(raw) => {
                if tokens
                    .peek()
                    .is_some_and(|t| matches!(t, Token::Code { .. }))
                {
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
            Token::Code { fence, code } => {
                if !pending.is_empty()
                    && let Some(Token::Text(next)) = tokens.peek()
                {
                    let (lead, mid, trail) = split_marks(next);
                    if mid.is_empty() && trail.is_empty() && lead == pending {
                        out.push_str(pending);
                        push_code(code, &mut out);
                        out.push_str(lead);
                        pending = "";
                        tokens.next();
                        continue;
                    }
                }
                let mut prefix = pending;
                let mut suffix = "";
                let mut modified = !pending.is_empty();
                pending = "";
                if let Some(Token::Text(next)) = tokens.peek_mut() {
                    let (lead, mid, _) = split_marks(next);
                    if !lead.is_empty() {
                        modified = true;
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
                if modified {
                    push_code(code, &mut out);
                } else {
                    out.push_str(fence);
                    out.push_str(code);
                    out.push_str(fence);
                }
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

    #[test]
    fn preserves_standalone_code() {
        let input = vec!["before `code` after".to_string()];
        assert_eq!(fix_code_emphasis(&input), input);
    }
}
