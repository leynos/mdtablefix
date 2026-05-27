//! Token and fragment predicates for inline Markdown wrapping.
//!
//! These helpers classify segmented tokens and rendered fragment text so span
//! grouping and post-wrap heuristics can recognise links, code, footnotes, and
//! punctuation without duplicating detection rules.

pub(in crate::wrap::inline) fn is_opening_punct(c: char) -> bool {
    matches!(c, '(' | '[') || "（［【《「『".contains(c)
}

pub(in crate::wrap::inline) fn is_trailing_punct(c: char) -> bool {
    // ASCII closers + common Unicode closers and word-final punctuation
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '"' | '\''
    ) || "…—–»›）］】》」』、。，：；！？”.’".contains(c)
}

/// Returns whether `token` already looks like a complete Markdown link.
pub(in crate::wrap::inline) fn looks_like_link(token: &str) -> bool {
    (token.starts_with('[') || token.starts_with("!["))
        && token.contains("](")
        && token.ends_with(')')
}

/// Returns whether `token` looks like a complete GFM footnote reference.
pub(in crate::wrap::inline) fn looks_like_footnote_ref(token: &str) -> bool {
    token
        .strip_prefix("[^")
        .and_then(|label| label.strip_suffix(']'))
        .is_some_and(|label| !label.is_empty())
}

/// Returns whether `token` ends with an inline footnote reference.
pub(in crate::wrap::inline) fn ends_with_footnote_ref(token: &str) -> bool {
    let Some(start) = token.rfind("[^") else {
        return false;
    };

    looks_like_footnote_ref(&token[start..])
}

/// Returns whether `token` contains only Unicode whitespace.
pub(in crate::wrap::inline) fn is_whitespace_token(token: &str) -> bool {
    token.chars().all(char::is_whitespace)
}

/// Returns whether `token` is a complete inline code span.
pub(in crate::wrap::inline) fn is_inline_code_token(token: &str) -> bool {
    token.len() > 1 && token.starts_with('`') && token.ends_with('`')
}

/// Returns the substring beginning at the first Markdown link opener after any
/// leading opener punctuation.
pub(in crate::wrap::inline) fn link_text_after_leading_openers(text: &str) -> &str {
    let mut rest = text;
    while !rest.is_empty() {
        if rest.starts_with('[') || rest.starts_with("![") {
            return rest;
        }
        let Some(ch) = rest.chars().next() else {
            break;
        };
        if is_opening_punct(ch) {
            rest = &rest[ch.len_utf8()..];
        } else {
            break;
        }
    }
    rest
}

/// Strips one outer wrapper closing character from a link candidate when present.
fn strip_outer_link_wrapper_suffix(text: &str) -> Option<&str> {
    let last = text.chars().next_back()?;
    if matches!(last, ')' | ']' | '）' | '］' | '」' | '』' | '》') {
        Some(&text[..text.len() - last.len_utf8()])
    } else {
        None
    }
}

/// Returns whether rendered fragment text contains a Markdown link, including
/// links wrapped in outer opener punctuation.
pub(in crate::wrap::inline) fn fragment_is_link(text: &str) -> bool {
    if looks_like_link(text) {
        return true;
    }
    let mut candidate = link_text_after_leading_openers(text);
    while !candidate.is_empty() {
        if looks_like_link(candidate) {
            return true;
        }
        let Some(next) = strip_outer_link_wrapper_suffix(candidate) else {
            break;
        };
        candidate = next;
    }
    false
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{
        is_inline_code_token,
        is_opening_punct,
        is_trailing_punct,
        is_whitespace_token,
        looks_like_footnote_ref,
    };

    fn backtick_run_strategy() -> BoxedStrategy<String> {
        prop::collection::vec(Just('`'), 1..8)
            .prop_map(|chars| chars.into_iter().collect::<String>())
            .boxed()
    }

    fn arbitrary_short_string_strategy() -> BoxedStrategy<String> {
        prop::collection::vec(any::<char>(), 0..24)
            .prop_map(|chars| chars.into_iter().collect::<String>())
            .boxed()
    }

    fn footnote_label_strategy() -> BoxedStrategy<String> {
        prop::string::string_regex("[a-zA-Z0-9_-]+")
            .unwrap()
            .boxed()
    }

    #[test]
    fn is_inline_code_token_rejects_lone_backtick_delimiter() {
        let delimiter = char::from(b'`');
        assert!(!is_inline_code_token(&delimiter.to_string()));
    }

    #[test]
    fn is_inline_code_token_accepts_complete_span() {
        let delimiter = char::from(b'`');
        let token = format!("{delimiter}code{delimiter}");
        assert!(is_inline_code_token(&token));
    }

    #[test]
    fn is_inline_code_token_matches_backtick_delimited_length_rule() {
        proptest!(|(token in backtick_run_strategy())| {
            let expected = token.len() > 1 && token.starts_with('`') && token.ends_with('`');
            prop_assert_eq!(is_inline_code_token(&token), expected);
        });
    }

    #[test]
    fn is_whitespace_token_matches_char_classification() {
        proptest!(|(token in arbitrary_short_string_strategy())| {
            prop_assert_eq!(
                is_whitespace_token(&token),
                token.chars().all(char::is_whitespace)
            );
        });
    }

    #[test]
    fn opening_and_trailing_punct_are_mutually_exclusive_for_ascii_letters() {
        for c in 'a'..='z' {
            assert!(!is_opening_punct(c));
            assert!(!is_trailing_punct(c));
        }
    }

    #[test]
    fn looks_like_footnote_ref_implies_non_empty_label() {
        proptest!(|(label in footnote_label_strategy())| {
            let token = format!("[^{label}]");
            prop_assert!(looks_like_footnote_ref(&token));
        });
    }

    #[test]
    fn looks_like_footnote_ref_rejects_empty_label() {
        assert!(!looks_like_footnote_ref("[^]"));
    }
}
