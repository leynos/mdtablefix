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

use std::borrow::Cow;

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

/// A fenced span whose content ends in a multibyte scalar, paired with the
/// synthetic offset that lands inside that scalar.
///
/// The trimmer records a trailing trim when the *last byte* of the content
/// range is synthetic, and that byte is the interior of the final `é`. The
/// resulting content slice therefore cannot be represented, which is the
/// checked-slice fallback rather than the prefilter exit: the content starts
/// with a space, so the text contains a backtick-then-space sequence and
/// `has_potential_synthetic_edge_space` is satisfied by construction.
fn span_with_mid_scalar_trailing_edge() -> impl Strategy<Value = (String, usize)> {
    span_with_trailing_edge("\u{e9}")
}

/// The same shape with an ASCII final content character.
///
/// It exists only as the control for the property above: the offset addresses
/// the identical logical position, so if this one trims and the multibyte one
/// does not, the difference is caused by the scalar split and nothing else.
fn span_with_ascii_trailing_edge() -> impl Strategy<Value = (String, usize)> {
    span_with_trailing_edge(" ")
}

/// Build `before` + fence + `" " + body + final_char` + fence + `after` and
/// report the byte offset of that final content character.
fn span_with_trailing_edge(final_char: &'static str) -> impl Strategy<Value = (String, usize)> {
    (spanless_text(), spanless_text(), spanless_text(), 1usize..4).prop_map(
        move |(before, body, after, fence_len)| {
            let fence = "`".repeat(fence_len);
            let content = format!(" {body}{final_char}");
            let text = format!("{before}{fence}{content}{fence}{after}");
            let content_end = before.len() + fence_len + content.len();
            (text, content_end - 1)
        },
    )
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

    /// A synthetic offset on the last byte of a span whose content ends in a
    /// multibyte scalar selects the trailing trim and then makes the content
    /// slice unrepresentable.
    ///
    /// This is the checked-slice fallback specifically, not the prefilter exit
    /// and not a mid-scalar curiosity: the offset is a real span edge, so the
    /// trimmer gets as far as building the slice, and the content is built with
    /// a leading space so the prefilter admits the text. The original must come
    /// back borrowed and untouched rather than rebuilt around half a character.
    #[test]
    fn trim_code_span_edge_spaces_borrows_text_when_a_span_edge_splits_a_scalar(
        (text, offset) in span_with_mid_scalar_trailing_edge(),
    ) {
        // Pin the preconditions in-property rather than trusting the
        // generator, which would otherwise silently test a different branch:
        // the trimmer selects its trailing trim from the *last* content byte,
        // and that byte must split a scalar for the content slice to be
        // unrepresentable. Reproduce the trimmer's own arithmetic to say so.
        let closing_fence_start = text
            .get(offset + 1..)
            .and_then(|tail| tail.find('`'))
            .map(|relative| offset + 1 + relative);
        prop_assert_eq!(
            closing_fence_start,
            Some(offset + 1),
            "offset {} in {:?} must be the last content byte, immediately before the closing fence",
            offset,
            text,
        );
        prop_assert!(
            !text.is_char_boundary(offset),
            "offset {} in {:?} must split a multibyte scalar",
            offset,
            text,
        );

        let trimmed = trim_code_span_edge_spaces(&text, &[offset]);

        prop_assert!(
            matches!(trimmed, Cow::Borrowed(original) if std::ptr::eq(original, text.as_str())),
            "edge offset {} splitting a scalar in {:?} must return the original text, got {:?}",
            offset,
            text,
            trimmed,
        );
    }

    /// The control for the property above: the same offset at the same logical
    /// position, on a span that ends in an ASCII space, must actually trim.
    ///
    /// Without this, the multibyte property would also pass if the generator
    /// had drifted into a shape that never reaches the trim at all. Together the
    /// two pin the honest claim: this offset position trims when it can, and
    /// declines when doing so would split a character.
    #[test]
    fn trim_code_span_edge_spaces_trims_the_same_edge_for_ascii(
        (text, offset) in span_with_ascii_trailing_edge(),
    ) {
        let trimmed = trim_code_span_edge_spaces(&text, &[offset]);

        prop_assert!(
            matches!(trimmed, Cow::Owned(_)) && trimmed.len() < text.len(),
            "edge offset {} in {:?} must trim, got {:?}",
            offset,
            text,
            trimmed,
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
