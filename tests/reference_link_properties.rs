//! Property tests for reference-style link wrapping invariants.

use mdtablefix::wrap::wrap_text;
use proptest::prelude::*;

fn reference_component_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            (b'a'..=b'z').prop_map(char::from),
            (b'A'..=b'Z').prop_map(char::from),
            (b'0'..=b'9').prop_map(char::from),
            Just('-'),
            Just('_'),
        ],
        1..=16,
    )
    .prop_map(|characters| characters.into_iter().collect())
}

fn prose_words_strategy() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec("[a-z]{1,12}", 0..=12)
}

proptest! {
    #[test]
    fn wrap_text_keeps_reference_links_atomic(
        label in reference_component_strategy(),
        reference_id in reference_component_strategy(),
        prefix in prose_words_strategy(),
        suffix in prose_words_strategy(),
        width in 8usize..=120,
    ) {
        let reference_link = format!("[{label}][{reference_id}]");
        let paragraph = prefix
            .iter()
            .chain(std::iter::once(&reference_link))
            .chain(suffix.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");

        let wrapped = wrap_text(&[paragraph], width);
        let rendered = wrapped.join("\n");

        prop_assert!(
            wrapped.iter().all(|line| !line.ends_with('[')),
            "a wrapped line ended with a bare opening bracket: {wrapped:?}",
        );
        prop_assert!(
            rendered.contains(&reference_link),
            "reference link {reference_link:?} was split in: {wrapped:?}",
        );
    }
}
