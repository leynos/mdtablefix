//! Token stream normalization before inline wrapping.
//!
//! The helpers in this module repair token boundaries that would otherwise
//! let the wrapping algorithm separate punctuation from the inline Markdown
//! construct it annotates.

use std::borrow::Cow;

use super::{is_trailing_punct, looks_like_footnote_ref};

/// Removes whitespace between trailing punctuation and an inline footnote ref.
///
/// This keeps sentence punctuation and an immediately following GFM footnote
/// reference as a single semantic unit before span building decides wrap
/// boundaries.
pub(in crate::wrap::inline) fn normalize_footnote_ref_spacing(
    tokens: &[String],
) -> Cow<'_, [String]> {
    let Some(first_match) =
        (0..tokens.len()).find(|index| matches_footnote_ref_spacing(tokens, *index))
    else {
        return Cow::Borrowed(tokens);
    };

    let mut normalized = Vec::with_capacity(tokens.len());
    normalized.extend_from_slice(&tokens[..first_match]);
    let mut index = first_match;

    while index < tokens.len() {
        if matches_footnote_ref_spacing(tokens, index) {
            normalized.push(tokens[index].clone());
            normalized.push(tokens[index + 2].clone());
            index += 3;
        } else {
            normalized.push(tokens[index].clone());
            index += 1;
        }
    }

    Cow::Owned(normalized)
}

fn matches_footnote_ref_spacing(tokens: &[String], index: usize) -> bool {
    tokens.get(index..index + 3).is_some_and(|window| {
        !looks_like_footnote_ref(&window[0])
            && window[0].chars().last().is_some_and(is_trailing_punct)
            && window[1].chars().all(char::is_whitespace)
            && looks_like_footnote_ref(&window[2])
    })
}

#[cfg(test)]
mod tests {
    //! Tests for inline footnote-reference normalization.

    use proptest::prelude::*;
    use rstest::rstest;

    use super::normalize_footnote_ref_spacing;
    use crate::wrap::inline::{is_trailing_punct, looks_like_footnote_ref};

    fn strings(tokens: &[&str]) -> Vec<String> { tokens.iter().map(ToString::to_string).collect() }

    fn footnote_label_strategy() -> impl Strategy<Value = String> {
        prop::string::string_regex("[a-zA-Z0-9_-]+")
            .expect("failed to build footnote label regex strategy")
    }

    fn footnote_ref_strategy() -> impl Strategy<Value = String> {
        footnote_label_strategy().prop_map(|label| format!("[^{label}]"))
    }

    fn footnote_definition_strategy() -> impl Strategy<Value = String> {
        footnote_label_strategy().prop_map(|label| format!("[^{label}]:"))
    }

    fn plain_token_strategy() -> impl Strategy<Value = String> {
        prop::string::string_regex("[A-Za-z0-9]{1,16}")
            .expect("failed to build plain token regex strategy")
    }

    fn punctuated_token_strategy() -> impl Strategy<Value = String> {
        (
            plain_token_strategy(),
            prop_oneof![
                Just("."),
                Just(","),
                Just(";"),
                Just(":"),
                Just("?"),
                Just("!"),
                Just(")"),
                Just("\""),
            ],
        )
            .prop_map(|(text, punct)| format!("{text}{punct}"))
    }

    fn whitespace_token_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(" ".to_string()),
            Just("  ".to_string()),
            Just("\n".to_string()),
            Just("\n  ".to_string()),
            Just("\t".to_string()),
        ]
    }

    fn arbitrary_token_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            plain_token_strategy(),
            punctuated_token_strategy(),
            whitespace_token_strategy(),
            footnote_ref_strategy(),
            footnote_definition_strategy(),
        ]
    }

    fn token_stream_strategy() -> impl Strategy<Value = Vec<String>> {
        prop::collection::vec(arbitrary_token_strategy(), 0..48)
    }

    fn should_remove_spacing(tokens: &[String], index: usize) -> bool {
        let Some(window) = tokens.get(index..index + 3) else {
            return false;
        };

        !looks_like_footnote_ref(&window[0])
            && window[0].chars().last().is_some_and(is_trailing_punct)
            && window[1].chars().all(char::is_whitespace)
            && looks_like_footnote_ref(&window[2])
    }

    fn removed_spacing_count(tokens: &[String]) -> usize {
        let mut removed = 0;
        let mut index = 0;

        while index < tokens.len() {
            if should_remove_spacing(tokens, index) {
                removed += 1;
                index += 3;
            } else {
                index += 1;
            }
        }

        removed
    }

    #[rstest]
    #[case::single_space(&["word.", " ", "[^1]"], &["word.", "[^1]"])]
    #[case::multiple_spaces(&["word.", "  ", "[^1]"], &["word.", "[^1]"])]
    #[case::newline(&["word.", "\n", "[^1]"], &["word.", "[^1]"])]
    #[case::newline_with_indent(&["word.", "\n  ", "[^1]"], &["word.", "[^1]"])]
    #[case::already_attached(&["word.", "[^1]"], &["word.", "[^1]"])]
    #[case::no_trailing_punctuation(&["word", " ", "[^1]"], &["word", " ", "[^1]"])]
    #[case::definition_token(&["word.", " ", "[^1]:"], &["word.", " ", "[^1]:"])]
    #[case::adjacent_references(&["a.", " ", "[^0]", " ", "[^_]"], &["a.", "[^0]", " ", "[^_]"])]
    fn normalizes_inline_footnote_ref_spacing(#[case] input: &[&str], #[case] expected: &[&str]) {
        assert_eq!(
            normalize_footnote_ref_spacing(&strings(input)).as_ref(),
            strings(expected)
        );
    }

    proptest! {
        #[test]
        fn normalizing_preserves_non_whitespace_tokens(tokens in token_stream_strategy()) {
            let normalized = normalize_footnote_ref_spacing(&tokens);
            let input_non_whitespace = tokens
                .iter()
                .filter(|token| !token.chars().all(char::is_whitespace))
                .collect::<Vec<_>>();
            let output_non_whitespace = normalized
                .iter()
                .filter(|token| !token.chars().all(char::is_whitespace))
                .collect::<Vec<_>>();

            prop_assert_eq!(output_non_whitespace, input_non_whitespace);
        }

        #[test]
        fn normalizing_removes_only_matched_spacing_tokens(tokens in token_stream_strategy()) {
            let normalized = normalize_footnote_ref_spacing(&tokens);

            prop_assert_eq!(
                normalized.len() + removed_spacing_count(&tokens),
                tokens.len()
            );
        }

        #[test]
        fn normalizing_is_idempotent(tokens in token_stream_strategy()) {
            let normalized = normalize_footnote_ref_spacing(&tokens);
            let renormalized = normalize_footnote_ref_spacing(&normalized);

            prop_assert_eq!(
                renormalized.as_ref(),
                normalized.as_ref()
            );
        }

        #[test]
        fn normalizing_collapses_generated_reference_pattern(
            prefix in token_stream_strategy(),
            punctuated in punctuated_token_strategy(),
            whitespace in whitespace_token_strategy(),
            reference in footnote_ref_strategy(),
            suffix in token_stream_strategy(),
        ) {
            let mut tokens = prefix;
            tokens.extend([punctuated.clone(), whitespace, reference.clone()]);
            tokens.extend(suffix);

            let normalized = normalize_footnote_ref_spacing(&tokens);

            prop_assert!(
                normalized
                    .windows(2)
                    .any(|window| window == [punctuated.clone(), reference.clone()]),
                "expected attached reference pair in {normalized:?}"
            );
        }

        #[test]
        fn normalizing_excludes_generated_definition_pattern(
            prefix in token_stream_strategy(),
            punctuated in punctuated_token_strategy(),
            whitespace in whitespace_token_strategy(),
            definition in footnote_definition_strategy(),
            suffix in token_stream_strategy(),
        ) {
            let mut tokens = prefix;
            tokens.extend([punctuated.clone(), whitespace.clone(), definition.clone()]);
            tokens.extend(suffix);

            let normalized = normalize_footnote_ref_spacing(&tokens);

            prop_assert!(
                normalized
                    .windows(3)
                    .any(|window| {
                        window == [
                            punctuated.clone(),
                            whitespace.clone(),
                            definition.clone(),
                        ]
                    }),
                "expected definition spacing to remain in {normalized:?}"
            );
        }
    }
}
