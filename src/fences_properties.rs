//! Test-only property-test companion to the `fences` module.
//!
//! These tests generate syntactically valid fence lines to verify the private
//! `Strategy` dispatch contract independently of matched-block tracking.
//! Kani is deliberately not used: the project has no Kani dev-dependency or
//! harness infrastructure, while these generated cases cover both finite
//! strategy states across the bounded marker lengths relevant to this module.

use proptest::{prelude::*, strategy::Strategy as ProptestStrategy};

use super::{Strategy, rewrite_marker};

fn fence_line_strategy() -> impl ProptestStrategy<Value = (String, String, char, usize, String)> {
    (
        prop::collection::vec(Just(' '), 0..=8),
        prop_oneof![Just('`'), Just('~')],
        3_usize..=10,
        prop_oneof![Just(String::new()), "[a-z][a-z0-9_+.-]{0,8}"],
    )
        .prop_map(|(indent, marker, marker_length, language)| {
            let indent: String = indent.into_iter().collect();
            let marker_run: String = std::iter::repeat_n(marker, marker_length).collect();
            let line = format!("{indent}{marker_run}{language}");
            (line, indent, marker, marker_length, language)
        })
}

proptest! {
    #[test]
    fn compress_writes_three_backticks_and_preserves_indent_and_language(
        (line, indent, _marker, _marker_length, language) in fence_line_strategy(),
    ) {
        let rewritten = rewrite_marker(&line, Strategy::Compress).expect("generated fence matches");

        prop_assert_eq!(rewritten, format!("{indent}```{language}"));
    }

    #[test]
    fn preserve_retains_the_original_marker_run(
        (line, indent, marker, marker_length, language) in fence_line_strategy(),
    ) {
        let rewritten = rewrite_marker(&line, Strategy::Preserve).expect("generated fence matches");
        let expected_marker: String = std::iter::repeat_n(marker, marker_length).collect();

        prop_assert_eq!(rewritten, format!("{indent}{expected_marker}{language}"));
    }

    #[test]
    fn rewriting_is_idempotent(
        (line, _indent, _marker, _marker_length, _language) in fence_line_strategy(),
    ) {
        for strategy in [Strategy::Compress, Strategy::Preserve] {
            let once = rewrite_marker(&line, strategy).expect("generated fence matches");
            let twice = rewrite_marker(&once, strategy).expect("rewritten fence matches");

            prop_assert_eq!(twice, once);
        }
    }
}
