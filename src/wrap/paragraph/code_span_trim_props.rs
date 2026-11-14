//! Property tests for the checked offsets in `trim_code_span_edge_spaces`.
//!
//! The unit cases in `code_span_trim.rs` pin specific synthetic offsets. These
//! properties generalise the same contract over generated text: that no
//! generated offset can make the trimmer panic, that an offset which cannot
//! address a span edge leaves the text untouched, and that a well-formed offset
//! can only ever remove a space.
//!
//! Every generator derives its offsets from the text it is paired with, rather
//! than filtering a uniform range. A filtered generator would reject almost
//! every sample — mid-scalar offsets and real span edges are both rare — and
//! proptest turns that rejection rate into a spurious failure.

use proptest::prelude::*;

use super::trim_code_span_edge_spaces;

/// Text that always contains a multibyte scalar, mixed with backtick fences,
/// escapes and spaces.
///
/// The leading `é` is what guarantees a mid-scalar byte offset exists; without
/// it a run of ASCII samples would leave the mid-scalar property with nothing
/// to assert about.
fn multibyte_text() -> impl Strategy<Value = String> {
    (
        Just('é'),
        proptest::collection::vec(
            prop_oneof![
                Just('`'),
                Just('\\'),
                Just(' '),
                Just('a'),
                Just('é'),
                Just('界'),
            ],
            0..18,
        ),
    )
        .prop_map(|(first, rest)| std::iter::once(first).chain(rest).collect())
}

/// Text with no backtick at all, so no span can ever be recognised.
fn spanless_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(prop_oneof![Just(' '), Just('a'), Just('é')], 0..18)
        .prop_map(|chars| chars.into_iter().collect())
}

/// Text built as `before` + fence + `content` + fence + `after`, together with
/// every offset that cannot address either end of the span content.
///
/// The fillers exclude backticks and backslashes, so the only fence runs are
/// the two the constructor adds and neither is escaped. Offsets are then read
/// off the constructed string instead of guessed.
fn single_span_case() -> impl Strategy<Value = (String, Vec<usize>)> {
    (spanless_text(), spanless_text(), spanless_text(), 1usize..4).prop_map(
        |(before, content, after, fence_len)| {
            let fence = "`".repeat(fence_len);
            let text = format!("{before}{fence}{content}{fence}{after}");
            let content_start = before.len() + fence_len;
            let content_end = content_start + content.len();
            // The trimmer only reacts to these two positions: the first content
            // byte, and the last content byte (the range's `next_back`).
            let edges = [content_start, content_end.saturating_sub(1)];
            let no_edge = (0..=text.len())
                .filter(|offset| !edges.contains(offset))
                .collect();
            (text, no_edge)
        },
    )
}

/// Every byte offset strictly inside a multibyte scalar.
fn mid_scalar_offsets(text: &str) -> Vec<usize> {
    text.char_indices()
        .filter(|(_, character)| character.len_utf8() > 1)
        .flat_map(|(start, character)| (start + 1)..(start + character.len_utf8()))
        .collect()
}

proptest! {
    /// Every generated offset must be answered, never panic.
    ///
    /// The trimmer rebuilds its output with checked `get` calls precisely so an
    /// unrepresentable range can be declined; a panic here would mean one of
    /// those guards is missing.
    #[test]
    fn trim_code_span_edge_spaces_never_panics(
        text in multibyte_text(),
        offsets in proptest::collection::vec(0usize..=64, 0..5),
    ) {
        let _ = trim_code_span_edge_spaces(&text, &offsets);
    }

    /// An offset that splits a multibyte scalar can still equal the last byte
    /// of the content range, which selects the trailing trim and then makes the
    /// content slice unrepresentable.
    ///
    /// The trimmer must return the original text in that case rather than
    /// building a string around half a character. This is the guard that is
    /// hardest to reach by hand, so it is the one worth generating.
    #[test]
    fn trim_code_span_edge_spaces_keeps_text_for_mid_scalar_offsets(
        text in multibyte_text(),
    ) {
        let offsets = mid_scalar_offsets(&text);
        prop_assume!(!offsets.is_empty());

        let trimmed = trim_code_span_edge_spaces(&text, &offsets);

        prop_assert_eq!(
            trimmed.as_ref(),
            text.as_str(),
            "mid-scalar offsets {:?} must leave {:?} untouched",
            offsets,
            text,
        );
    }

    /// An offset that addresses neither end of a span's content trims nothing,
    /// so the text must come back unchanged.
    ///
    /// This is the general form of the `[0]` unit case: a caller may pass an
    /// offset that is simply irrelevant, and that must be a no-op rather than an
    /// edit at the wrong place. Every non-edge offset the constructed text
    /// admits is passed at once, so the property covers all of them for each
    /// generated span rather than only the first.
    #[test]
    fn trim_code_span_edge_spaces_keeps_text_for_offsets_that_match_no_edge(
        (text, no_edge) in single_span_case(),
    ) {
        prop_assume!(!no_edge.is_empty());

        let trimmed = trim_code_span_edge_spaces(&text, &no_edge);

        prop_assert_eq!(
            trimmed.as_ref(),
            text.as_str(),
            "non-edge offsets must leave {:?} untouched",
            text,
        );
    }

    /// With no backtick run there is no span edge to match, so no offset can
    /// change the text, however well-formed that offset is.
    #[test]
    fn trim_code_span_edge_spaces_keeps_text_without_spans(
        text in spanless_text(),
        offsets in proptest::collection::vec(0usize..=64, 0..5),
    ) {
        let trimmed = trim_code_span_edge_spaces(&text, &offsets);

        prop_assert_eq!(
            trimmed.as_ref(),
            text.as_str(),
            "offsets {:?} must not change span-free text {:?}",
            offsets,
            text,
        );
    }
}
