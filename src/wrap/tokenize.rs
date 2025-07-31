//! Tokenization helpers for wrapping logic.
//!
//! This module contains utilities for breaking lines into tokens so that
//! inline code spans and Markdown links are preserved during wrapping.

use super::FENCE_RE;

fn scan_while<F>(chars: &[char], mut i: usize, cond: F) -> usize
where
    F: Fn(char) -> bool,
{
    while i < chars.len() && cond(chars[i]) {
        i += 1;
    }
    i
}

fn collect_range(chars: &[char], start: usize, end: usize) -> String {
    chars[start..end].iter().collect()
}

/// Markdown token emitted by [`tokenize_markdown`].
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

/// Parse a Markdown link or image starting at `i`.
///
/// Handles nested parentheses within URLs by tracking the depth of opening and
/// closing delimiters. Returns the parsed slice and the index after the closing
/// parenthesis if one is found.
fn parse_link_or_image(chars: &[char], mut i: usize) -> (String, usize) {
    let start = i;
    if chars[i] == '!' {
        i += 1;
    }
    i += 1; // skip initial '[' which we know is present
    i = scan_while(chars, i, |c| c != ']');
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
            return (collect_range(chars, start, i), i);
        }
    }
    (collect_range(chars, start, start + 1), start + 1)
}

fn is_trailing_punctuation(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '"' | '\''
    )
}

pub(super) fn segment_inline(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            let start = i;
            i = scan_while(&chars, i, char::is_whitespace);
            tokens.push(collect_range(&chars, start, i));
        } else if c == '`' {
            let start = i;
            let fence_end = scan_while(&chars, i, |ch| ch == '`');
            let fence_len = fence_end - start;
            i = fence_end;

            let mut end = i;
            while end < chars.len() {
                let j = scan_while(&chars, end, |ch| ch == '`');
                if j - end == fence_len {
                    end = j;
                    break;
                }
                end += 1;
            }

            if end >= chars.len() {
                tokens.push(collect_range(&chars, start, start + fence_len));
                i = start + fence_len;
            } else {
                tokens.push(collect_range(&chars, start, end));
                i = end;
            }
        } else if c == '[' || (c == '!' && i + 1 < chars.len() && chars[i + 1] == '[') {
            let (tok, mut new_i) = parse_link_or_image(&chars, i);
            tokens.push(tok);
            let punct_start = new_i;
            new_i = scan_while(&chars, new_i, is_trailing_punctuation);
            if new_i > punct_start {
                tokens.push(collect_range(&chars, punct_start, new_i));
            }
            i = new_i;
        } else {
            let start = i;
            i = scan_while(&chars, i, |ch| !ch.is_whitespace() && ch != '`');
            tokens.push(collect_range(&chars, start, i));
        }
    }
    tokens
}

/// Split the input string into [`Token`]s by analysing whitespace and backtick
/// delimiters.
///
/// The tokenizer groups consecutive whitespace into a single [`Token::Text`] and
/// recognises backtick sequences as inline code spans. When a run of backticks
/// is encountered the parser searches forward for an identical delimiter,
/// allowing nested backticks when the span uses a longer fence. Unmatched
/// delimiter sequences are treated as literal text.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_with_trailing_punctuation() {
        let tokens = segment_inline("see [link](url).");
        assert_eq!(tokens, vec!["see", " ", "[link](url)", "."]);
    }

    #[test]
    fn image_with_nested_parentheses() {
        let tokens = segment_inline("![alt](path(a(b)c))");
        assert_eq!(tokens, vec!["![alt](path(a(b)c))"]);
    }

    #[test]
    fn inline_code_fences() {
        let tokens = segment_inline("use ``cmd`` now");
        assert_eq!(tokens, vec!["use", " ", "``cmd``", " ", "now"]);
    }

    #[test]
    fn unmatched_backticks() {
        let tokens = segment_inline("bad `code span");
        assert_eq!(tokens, vec!["bad", " ", "`", "code", " ", "span"]);
    }
}
