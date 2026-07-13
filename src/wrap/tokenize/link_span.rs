//! Locate balanced inline-link and image spans without allocating token text.
//!
//! The helper composes the tokenizer's existing label and destination parsers
//! so internal transforms can share Markdown boundary rules without exposing a
//! new public token variant or duplicating parenthesis handling.

use std::ops::Range;

use super::{
    parsing::{parse_link_text, parse_link_url},
    scanning::has_odd_backslash_escape_bytes,
};

/// Return the complete source span for an inline link or image.
///
/// Escaped openers and reference labels without an inline destination are not
/// classified. The returned range borrows the caller's original source.
pub(crate) fn link_or_image_span(text: &str, start: usize) -> Option<Range<usize>> {
    if start >= text.len() || has_odd_backslash_escape_bytes(text.as_bytes(), start) {
        return None;
    }

    let label_start = if text[start..].starts_with('!') {
        start + '!'.len_utf8()
    } else {
        start
    };
    let text_end = parse_link_text(text, label_start)?;
    let url_end = parse_link_url(text, text_end)?;
    Some(start..url_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case::link("[label](destination)", Some(0..20))]
    #[case::image("![alt](image(a).png)", Some(0..20))]
    #[case::reference("[label]", None)]
    #[case::escaped(r"\[label](destination)", None)]
    fn locates_complete_span(#[case] input: &str, #[case] expected: Option<Range<usize>>) {
        assert_eq!(link_or_image_span(input, 0), expected);
    }
}
