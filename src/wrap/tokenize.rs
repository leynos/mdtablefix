//! Tokenization helpers for wrapping logic.
//!
//! This module contains utilities for breaking lines into tokens so that
//! inline code spans and Markdown links are preserved during wrapping.

/// Advance `i` while the predicate evaluates to `true`.
///
/// Returns the index of the first character for which `cond` fails. This small
/// helper keeps the scanning loops concise.
///
/// # Examples
///
/// ```rust,ignore
/// let chars: Vec<char> = "abc123".chars().collect();
/// let end = scan_while(&chars, 0, char::is_alphabetic);
/// assert_eq!(end, 3);
/// ```
fn scan_while<F>(chars: &[char], mut i: usize, mut cond: F) -> usize
where
    F: FnMut(char) -> bool,
{
    while i < chars.len() && cond(chars[i]) {
        i += 1;
    }
    i
}

/// Collect a range of characters into a [`String`].
///
/// # Examples
///
/// ```rust,ignore
/// let chars: Vec<char> = ['a', 'b', 'c'];
/// assert_eq!(collect_range(&chars, 0, 2), "ab");
/// ```
fn collect_range(chars: &[char], start: usize, end: usize) -> String {
    chars[start..end].iter().collect()
}

/// Markdown token emitted by [`segment_inline`].
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
///
/// # Examples
///
/// ```rust,ignore
/// let chars: Vec<char> = "![alt](a(b)c)".chars().collect();
/// let (tok, idx) = parse_link_or_image(&chars, 0);
/// assert_eq!(tok, "![alt](a(b)c)");
/// assert_eq!(idx, chars.len());
/// ```
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

/// Determine whether a character is considered trailing punctuation.
///
/// The wrapper treats such punctuation as part of the preceding link when
/// wrapping lines.
///
/// # Examples
///
/// ```rust,ignore
/// assert!(is_trailing_punctuation('.'));
/// assert!(is_trailing_punctuation('('));
/// assert!(!is_trailing_punctuation('a'));
/// ```
fn is_trailing_punctuation(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | '(' | ')' | ']' | '"' | '\''
    )
}

/// Break a single line of text into inline token strings.
///
/// Code spans, links, images and surrounding whitespace are preserved as
/// separate tokens. This simplifies later wrapping logic which operates on
/// slices of the original text.
///
/// # Examples
///
/// ```rust,ignore
/// let tokens = segment_inline("see [link](url) and `code`");
/// assert_eq!(
///     tokens,
///     vec!["see", " ", "[link](url)", " ", "and", " ", "`code`"]
/// );
///
/// // Example with consecutive and unusual whitespace
/// let tokens = segment_inline("foo  bar\tbaz   `qux`");
/// assert_eq!(
///     tokens,
///     vec!["foo", "  ", "bar", "\t", "baz", "   ", "`qux`"]
/// );
/// ```
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

fn next_token(s: &str) -> Option<(Token<'_>, usize)> {
    if s.is_empty() {
        return None;
    }
    let delim_len = s.chars().take_while(|&c| c == '`').count();
    if delim_len == 0 {
        if let Some(pos) = s.find('`') {
            return Some((Token::Text(&s[..pos]), pos));
        }
        return Some((Token::Text(s), s.len()));
    }

    let closing = &s[..delim_len];
    if let Some(end) = s[delim_len..].find(closing) {
        let code = &s[delim_len..delim_len + end];
        return Some((Token::Code(code), delim_len + end + delim_len));
    }
    Some((Token::Text(closing), delim_len))
}

/// Emit [`Token`]s for inline segments within a single line.
///
/// The function scans for backtick sequences and yields `Token::Code` for
/// matched spans. Text outside code spans is emitted as `Token::Text` via the
/// provided callback.
///
/// # Examples
///
/// ```rust,ignore
/// // Prints:
/// // Token::Text("run ")
/// // Token::Code("cmd")
/// tokenize_inline("run `cmd`", &mut |t| println!("{:?}", t));
/// ```
///
/// The callback receives each token as a [`Token<'a>`], such as
/// `Token::Text(&str)` or `Token::Code(&str)`.
fn tokenize_inline<'a, F>(mut rest: &'a str, mut emit: F)
where
    F: FnMut(Token<'a>),
{
    while let Some((tok, used)) = next_token(rest) {
        emit(tok);
        rest = &rest[used..];
        if rest.is_empty() {
            break;
        }
    }
}

/// Tokenize a Markdown snippet using backtick-delimited code spans.
///
/// The function scans the input line by line. Lines matching [`FENCE_RE`]
/// produce [`Token::Fence`] tokens and toggle fenced mode. Lines inside a
/// fence are yielded verbatim. Outside fenced regions the scanner searches for
/// backtick sequences. Text before a backtick becomes [`Token::Text`]. When a
/// closing backtick follows, the enclosed portion forms a [`Token::Code`]
/// span. If no closing backtick is found the delimiter and remaining text are
/// returned as [`Token::Text`]. Whitespace is preserved exactly as it appears.
///
/// ```rust
/// use crate::wrap::{Token, tokenize_markdown};
///
/// let tokens = tokenize_markdown("Example with `code`");
/// assert_eq!(
///     tokens,
///     vec![Token::Text("Example with "), Token::Code("code")]
/// );
/// ```
#[must_use]
pub fn tokenize_markdown(source: &str) -> Vec<Token<'_>> {
    if source.is_empty() {
        return Vec::new();
    }

    let mut tokens = Vec::new();
    let lines: Vec<&str> = source.split('\n').collect();
    let last_idx = lines.len() - 1;
    let mut in_fence = false;

    for (i, line) in lines.iter().enumerate() {
        if super::is_fence(line) {
            tokens.push(Token::Fence(line));
            if i != last_idx {
                tokens.push(Token::Newline);
            }
            in_fence = !in_fence;
            continue;
        }

        if in_fence {
            tokens.push(Token::Fence(line));
            if i != last_idx {
                tokens.push(Token::Newline);
            }
            continue;
        }

        tokenize_inline(line, &mut |tok| tokens.push(tok));
        if i != last_idx {
            tokens.push(Token::Newline);
        }
    }
    tokens
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
