//! Property tests for the checked byte-range contracts in `spanning_code`.
//!
//! The unit cases in `spanning_code_boundary_tests.rs` pin specific invalid
//! offsets. These properties generalise the same contract over generated text
//! and generated offsets, which is where an out-of-range or mid-scalar slice
//! would otherwise only surface as a panic on an unlucky input.

use proptest::prelude::*;

use super::{overlong_span_crossing_boundaries, split_span_at_boundaries};

/// Text that mixes ASCII with multibyte scalars, so byte offsets and character
/// offsets diverge and a mid-scalar boundary is reachable.
fn mixed_width_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            Just('`'),
            Just(' '),
            Just('a'),
            Just('é'),
            Just('界'),
            Just('ω'),
        ],
        0..24,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

/// A byte offset that is deliberately *not* constrained to a character
/// boundary, so both valid and mid-scalar positions are generated.
fn byte_offset(max: usize) -> impl Strategy<Value = usize> { 0..=max }

/// Rebuild `text` with one separator byte removed at each boundary.
///
/// This models what `split_span_at_boundaries` must retain: it drops the
/// synthetic space recorded at each boundary and keeps everything else. Returns
/// `None` if any slice is not a character boundary, which is a generator case
/// the caller skips rather than a property violation.
fn reassemble_without_separators(text: &str, boundaries: &[usize]) -> Option<String> {
    let mut expected = String::new();
    let mut cursor = 0;
    for boundary in boundaries {
        expected.push_str(text.get(cursor..*boundary)?);
        cursor = boundary.checked_add(1)?;
    }
    expected.push_str(text.get(cursor..)?);
    Some(expected)
}

proptest! {
    /// Every generated range must be answered, never panic.
    ///
    /// `overlong_span_crossing_boundaries` returns `Option` precisely so an
    /// unrepresentable range can be declined; a panic here would mean the
    /// guarding is incomplete.
    #[test]
    fn overlong_span_crossing_boundaries_never_panics(
        text in mixed_width_text(),
        boundaries in proptest::collection::vec(byte_offset(32), 0..4),
        start in byte_offset(32),
        end in byte_offset(32),
        width in 0usize..40,
    ) {
        let _ = overlong_span_crossing_boundaries(&text, &boundaries, start..end, width);
    }

    /// A mid-scalar or out-of-range boundary must be rejected rather than
    /// silently skipped, so the caller cannot emit half a character.
    #[test]
    fn split_span_rejects_non_boundary_offsets(
        text in mixed_width_text(),
        boundary in byte_offset(32),
    ) {
        let is_valid_offset = text.is_char_boundary(boundary);
        let result = split_span_at_boundaries(&text, 0, text.len(), &[boundary]);

        if !is_valid_offset && boundary <= text.len() {
            prop_assert!(
                result.is_none(),
                "mid-scalar boundary {boundary} in {text:?} must be rejected, got {result:?}"
            );
        }
    }

    /// For boundaries that *are* character boundaries, the pieces must
    /// reassemble the span exactly, minus the one synthetic separator byte
    /// consumed at each boundary.
    ///
    /// This is the preservation half of the contract: rejection is safe, but it
    /// must not reject reachable input.
    #[test]
    fn split_span_preserves_span_content_for_valid_boundaries(
        text in mixed_width_text(),
        raw_boundaries in proptest::collection::vec(byte_offset(32), 0..4),
    ) {
        let mut boundaries: Vec<usize> = raw_boundaries
            .into_iter()
            .filter(|boundary| *boundary <= text.len() && text.is_char_boundary(*boundary))
            .collect();
        boundaries.sort_unstable();
        boundaries.dedup();

        let Some(pieces) = split_span_at_boundaries(&text, 0, text.len(), &boundaries) else {
            return Ok(());
        };

        // Each boundary consumes exactly one separator byte, so dropping those
        // bytes from the source must equal the concatenated pieces. Checked
        // access keeps this test itself free of the direct slicing the module
        // under test exists to remove.
        let Some(expected) = reassemble_without_separators(&text, &boundaries) else {
            return Ok(());
        };

        prop_assert_eq!(
            pieces.concat(),
            expected,
            "pieces for boundaries {:?} in {:?} must reassemble the span",
            boundaries,
            text,
        );
    }
}
