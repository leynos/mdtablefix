//! Inline token segmentation for a single line of Markdown.
//!
//! These helpers split one line into the tokens that later wrapping treats as
//! atomic: whitespace runs, backtick code spans with any inflectional suffix,
//! links and images with their trailing punctuation, and plain text. They live
//! beside `Token` rather than inside it so the public tokenizer and its fence
//! handling stay readable, and because the segmentation is the part that
//! changes when inline Markdown grammar does.
//!
//! Every function here borrows the source line and allocates only the tokens it
//! emits; nothing inspects or mutates surrounding state.

use super::{
    parsing::{
        handle_backtick_fence,
        is_trailing_punctuation,
        looks_like_image_start,
        parse_link_or_image,
    },
    scanning::{
        bracket_follows_escaped_bang,
        collect_range,
        has_odd_backslash_escape_bytes,
        scan_code_suffix_end,
        scan_while,
    },
};

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
#[cfg(test)]
pub(in crate::wrap) fn segment_inline(text: &str) -> Vec<String> {
    let mut observer = crate::wrap::observer::NoOpObserver;
    segment_inline_observed(text, &mut Some(&mut observer))
}

/// Segments `text` into inline tokens, reporting parsing events.
///
/// Identical to [`segment_inline`] except that link, image, and footnote
/// parsing outcomes are reported to `observer`; with a `None` handle the
/// segmentation is the same and nothing is emitted.
pub(in crate::wrap) fn segment_inline_observed(
    text: &str,
    observer: &mut crate::wrap::observer::ObserverHandle<'_>,
) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < text.len() {
        i = push_token_at(text, i, &mut tokens, observer);
    }
    tokens
}

/// Appends the whitespace run starting at `i` and returns the next offset.
fn push_whitespace_run(text: &str, i: usize, tokens: &mut Vec<String>) -> usize {
    let end = scan_while(text, i, char::is_whitespace);
    tokens.push(collect_range(text, i, end));
    end
}

/// Appends the backtick-delimited token starting at `i`.
///
/// `ch` is the backtick at `i`, already decoded by the caller.
fn push_backtick_token(text: &str, i: usize, ch: char, tokens: &mut Vec<String>) -> usize {
    if has_odd_backslash_escape_bytes(text.as_bytes(), i) {
        append_escaped_backtick(tokens);
        return i + ch.len_utf8();
    }

    let (token, end) = handle_backtick_fence(text, i);
    let (token, end) = extend_closed_code_token(text, i, token, end);
    tokens.push(token);
    end
}

/// Returns whether a link or image token starts at `i`.
fn is_link_or_image_start(text: &str, i: usize, ch: char) -> bool {
    let looks_like_image = looks_like_image_start(text, i, ch);
    (ch == '[' || looks_like_image) && !has_odd_backslash_escape_bytes(text.as_bytes(), i)
}

/// Appends the link or image token starting at `i`, plus its punctuation.
fn push_link_or_image(
    text: &str,
    i: usize,
    tokens: &mut Vec<String>,
    observer: &mut crate::wrap::observer::ObserverHandle<'_>,
) -> usize {
    let (token, link_end) = parse_link_or_image(text, i, observer);
    tokens.push(token);
    let punctuation_end = scan_trailing_punctuation_end(text, link_end);
    if punctuation_end > link_end {
        tokens.push(collect_range(text, link_end, punctuation_end));
    }
    punctuation_end
}

/// Appends the plain-text run starting at `i` and returns the next offset.
fn push_plain_text(text: &str, i: usize, tokens: &mut Vec<String>) -> usize {
    let end = scan_plain_text_end(text, text.as_bytes(), i);
    tokens.push(collect_range(text, i, end));
    end
}

/// Appends the token that begins at byte `i` and returns the next byte offset.
///
/// The branch order is significant: whitespace and backticks are recognised
/// before the link-and-image test, because a backtick run can contain brackets
/// that would otherwise parse as a link.
fn push_token_at(
    text: &str,
    i: usize,
    tokens: &mut Vec<String>,
    observer: &mut crate::wrap::observer::ObserverHandle<'_>,
) -> usize {
    let Some(ch) = text[i..].chars().next() else {
        // Unreachable for callers whose loop condition is `i < text.len()`;
        // returning the end keeps a malformed slice from spinning forever.
        return text.len();
    };
    if ch.is_whitespace() {
        return push_whitespace_run(text, i, tokens);
    }
    if ch == '`' {
        return push_backtick_token(text, i, ch, tokens);
    }
    if is_link_or_image_start(text, i, ch) {
        return push_link_or_image(text, i, tokens, observer);
    }
    push_plain_text(text, i, tokens)
}

/// Scan punctuation following a link without absorbing an inline citation.
///
/// Keeping `([` as a new token boundary preserves citation grouping during
/// wrapping while ordinary trailing punctuation remains attached to the link.
fn scan_trailing_punctuation_end(text: &str, mut index: usize) -> usize {
    while index < text.len() {
        let Some(current) = text[index..].chars().next() else {
            break;
        };
        if starts_inline_citation(text, index) {
            break;
        }
        if !is_trailing_punctuation(current) {
            break;
        }
        index += current.len_utf8();
    }
    index
}

/// Recognise the opening delimiter of an inline parenthetical citation.
///
/// The tokenizer treats this pair specially so punctuation scanning cannot
/// consume the parenthesis that gives the citation its Markdown boundary.
fn starts_inline_citation(text: &str, index: usize) -> bool {
    text.get(index..).is_some_and(|tail| tail.starts_with("(["))
}

/// Keep an escaped backtick in the adjoining text token.
///
/// It is literal payload, never a code-fence boundary, and joining it avoids
/// creating a zero-width token merely because the source used an escape.
fn append_escaped_backtick(tokens: &mut Vec<String>) {
    if let Some(last) = tokens.last_mut() {
        last.push('`');
    } else {
        tokens.push(String::from("`"));
    }
}

/// Find the next inline construct boundary in otherwise ordinary text.
///
/// The scan stops before unescaped links, images, citations, or code fences so
/// later parsing can preserve each construct as an atomic wrapping unit.
fn scan_plain_text_end(text: &str, bytes: &[u8], mut index: usize) -> usize {
    if starts_inline_citation(text, index) && !has_odd_backslash_escape_bytes(bytes, index) {
        return index + 1;
    }

    while index < text.len() {
        let Some(current) = text[index..].chars().next() else {
            break;
        };
        if current.is_whitespace() || current == '`' {
            break;
        }

        let current_escaped = has_odd_backslash_escape_bytes(bytes, index);
        if should_stop_plain_text(text, bytes, index, (current, current_escaped)) {
            break;
        }

        index += current.len_utf8();
    }
    index
}

/// Decide whether the current character begins an atomic inline construct.
///
/// Escaped delimiters remain text; only real link, image, and citation openers
/// stop the plain-text scan and hand control to their dedicated parser.
fn should_stop_plain_text(text: &str, bytes: &[u8], index: usize, current: (char, bool)) -> bool {
    let (ch, is_escaped) = current;
    if ch == '[' {
        return !is_escaped
            && !bracket_follows_escaped_bang(bytes, index)
            && !bracket_follows_escaped_open_paren(bytes, index);
    }
    if ch == '(' {
        return !is_escaped && starts_inline_citation(text, index);
    }
    looks_like_image_start(text, index, ch) && !is_escaped
}

/// Recognise a bracket protected by an escaped parenthesis immediately before it.
///
/// That sequence is literal text, so treating its bracket as a link opener
/// would split a token where Markdown escaping promised it would not.
fn bracket_follows_escaped_open_paren(bytes: &[u8], index: usize) -> bool {
    index.checked_sub(1).is_some_and(|previous| {
        bytes[previous] == b'(' && has_odd_backslash_escape_bytes(bytes, previous)
    })
}

/// Confirm that a token begins and ends with the same non-empty code fence.
///
/// Suffix absorption is valid only for a completed span; an opener without its
/// closer must remain ordinary text for the continuation logic to resolve.
pub(super) fn is_closed_inline_code_span(token: &str) -> bool {
    let fence_len = token.chars().take_while(|&ch| ch == '`').count();
    fence_len > 0 && token.len() > fence_len * 2 && token.ends_with(&"`".repeat(fence_len))
}

/// Extend a completed code span through an attached inflectional suffix.
///
/// The suffix stays atomic with the span so a line break cannot change how
/// Markdown readers associate punctuation or prose with the code literal.
fn extend_closed_code_token(
    text: &str,
    start: usize,
    token: String,
    code_end: usize,
) -> (String, usize) {
    if !is_closed_inline_code_span(&token) {
        return (token, code_end);
    }
    let suffix_end = scan_code_suffix_end(text, code_end);
    if suffix_end > code_end {
        (collect_range(text, start, suffix_end), suffix_end)
    } else {
        (token, code_end)
    }
}
