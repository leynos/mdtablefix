//! Token stream normalisation before inline wrapping.
//!
//! The helpers in this module repair token boundaries that would otherwise
//! let the wrapping algorithm separate punctuation from the inline Markdown
//! construct it annotates.

use super::{is_trailing_punct, looks_like_footnote_ref};

/// Removes whitespace between trailing punctuation and an inline footnote ref.
///
/// This keeps sentence punctuation and an immediately following GFM footnote
/// reference as a single semantic unit before span building decides wrap
/// boundaries.
pub(in crate::wrap::inline) fn normalize_footnote_ref_spacing(tokens: &[String]) -> Vec<String> {
    let mut normalized = Vec::with_capacity(tokens.len());
    let mut index = 0;

    while index < tokens.len() {
        if should_skip_footnote_ref_spacing(tokens, index) {
            normalized.push(tokens[index].clone());
            normalized.push(tokens[index + 2].clone());
            index += 3;
        } else {
            normalized.push(tokens[index].clone());
            index += 1;
        }
    }

    normalized
}

fn should_skip_footnote_ref_spacing(tokens: &[String], index: usize) -> bool {
    tokens
        .get(index..index + 3)
        .is_some_and(is_footnote_ref_spacing)
}

fn is_footnote_ref_spacing(tokens: &[String]) -> bool {
    tokens[0].chars().last().is_some_and(is_trailing_punct)
        && tokens[1].chars().all(char::is_whitespace)
        && looks_like_footnote_ref(&tokens[2])
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::normalize_footnote_ref_spacing;

    fn strings(tokens: &[&str]) -> Vec<String> { tokens.iter().map(ToString::to_string).collect() }

    #[rstest]
    #[case::single_space(&["word.", " ", "[^1]"], &["word.", "[^1]"])]
    #[case::multiple_spaces(&["word.", "  ", "[^1]"], &["word.", "[^1]"])]
    #[case::newline(&["word.", "\n", "[^1]"], &["word.", "[^1]"])]
    #[case::newline_with_indent(&["word.", "\n  ", "[^1]"], &["word.", "[^1]"])]
    #[case::already_attached(&["word.", "[^1]"], &["word.", "[^1]"])]
    #[case::no_trailing_punctuation(&["word", " ", "[^1]"], &["word", " ", "[^1]"])]
    #[case::definition_token(&["word.", " ", "[^1]:"], &["word.", " ", "[^1]:"])]
    fn normalizes_inline_footnote_ref_spacing(#[case] input: &[&str], #[case] expected: &[&str]) {
        assert_eq!(
            normalize_footnote_ref_spacing(&strings(input)),
            strings(expected)
        );
    }
}
