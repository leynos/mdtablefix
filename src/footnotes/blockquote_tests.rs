//! Property tests for blockquote-marker normalization.
//!
//! These tests exercise the crate-private canonical helper directly because
//! integration tests cannot access the intentionally internal API.

use proptest::prelude::*;

use super::strip_blockquote_markers;

/// Builds arbitrary-depth blockquote prefixes with varied whitespace.
fn blockquote_prefix_strategy() -> BoxedStrategy<String> {
    let whitespace = prop::sample::select(vec!["", " ", "\t", "\n", "\r", "\u{2003}"]);
    prop::collection::vec((whitespace.clone(), whitespace), 0..=32)
        .prop_map(|markers| {
            markers
                .into_iter()
                .fold(String::new(), |mut prefix, (before, after)| {
                    prefix.push_str(before);
                    prefix.push('>');
                    prefix.push_str(after);
                    prefix
                })
        })
        .boxed()
}

/// Builds non-marker suffixes whose first character cannot be stripped.
fn arbitrary_suffix_strategy() -> BoxedStrategy<String> {
    prop::string::string_regex("[A-Za-z0-9][^>]{0,128}")
        .expect("suffix strategy regex should compile")
        .boxed()
}

proptest! {
    #[test]
    fn strip_blockquote_markers_is_idempotent(input in any::<String>()) {
        let stripped = strip_blockquote_markers(&input);
        prop_assert_eq!(strip_blockquote_markers(stripped), stripped);
    }

    #[test]
    fn strip_blockquote_markers_removes_leading_markers(input in any::<String>()) {
        prop_assert!(!strip_blockquote_markers(&input).starts_with('>'));
    }

    #[test]
    fn strip_blockquote_markers_preserves_suffix(
        prefix in blockquote_prefix_strategy(),
        suffix in arbitrary_suffix_strategy(),
    ) {
        let input = format!("{prefix}{suffix}");
        prop_assert_eq!(strip_blockquote_markers(&input), suffix);
    }
}
