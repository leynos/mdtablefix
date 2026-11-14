//! Property tests for the checked byte-range contracts in `spanning_code`.
//!
//! The unit cases in `spanning_code_boundary_tests.rs` pin specific invalid
//! offsets. These properties generalise the same contract over generated text
//! and generated offsets, which is where an out-of-range or mid-scalar slice
//! would otherwise only surface as a panic on an unlucky input.
//!
//! Each generator constructs the condition it needs — a mid-scalar offset, an
//! absent span — instead of filtering a uniform range. A filtered generator
//! rejects most samples and proptest reports that rejection rate as a failure,
//! which would make the suite flaky rather than rigorous.

use proptest::{prelude::*, sample::select};
use unicode_width::UnicodeWidthStr;

use super::{
    OverlongSpan,
    ProseEdge,
    overlong_span_crossing_boundaries,
    preserve_span_boundaries,
    reattach_prose,
    split_span_at_boundaries,
};

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

/// Text containing a backtick fence, at least one multibyte scalar, and one
/// byte offset strictly inside that scalar.
///
/// The offset is derived from the constructed string, so every sample is a
/// genuine mid-scalar position rather than a filter result.
fn joining_text_with_mid_scalar_offset() -> impl Strategy<Value = (String, usize)> {
    (mixed_width_text(), mixed_width_text()).prop_map(|(before, after)| {
        // The leading scalar guarantees a mid-scalar byte exists even when the
        // generated halves are empty or all-ASCII.
        let text = format!("{before}é{after}");
        let multibyte_start = before.len();
        (text, multibyte_start + 1)
    })
}

/// A joined text, a span range that is a valid slice of it, and wrapped lines
/// that cannot contain that span.
///
/// The span is fenced, while the lines are built only from `z` and spaces, so
/// the two alphabets cannot overlap and the `span_lookup_miss` branch is
/// reached on every sample.
fn absent_span_case() -> impl Strategy<Value = (String, usize, usize, Vec<String>)> {
    proptest::collection::vec(prop_oneof![Just('a'), Just('b'), Just('é')], 1..8).prop_map(
        |content| {
            let content: String = content.into_iter().collect();
            let joined = format!("`{content}`");
            let lines = vec!["zz zz".to_owned()];
            (joined, 0, content.len() + 2, lines)
        },
    )
}

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
        boundaries in proptest::collection::vec(0usize..=32, 0..4),
        start in 0usize..=32,
        end in 0usize..=32,
        width in 0usize..40,
    ) {
        let _ = overlong_span_crossing_boundaries(&text, &boundaries, start..end, width);
    }

    /// A mid-scalar or out-of-range boundary must be rejected rather than
    /// silently skipped, so the caller cannot emit half a character.
    #[test]
    fn split_span_rejects_non_boundary_offsets(text in mixed_width_text(), boundary in 0usize..=32) {
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
        raw_boundaries in proptest::collection::vec(0usize..=32, 0..4),
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

    /// Replacing a preserved span must be answered for every range, never
    /// panic.
    ///
    /// `preserve_span_boundaries` searches the wrapped lines and slices them
    /// around the span, so an unguarded range would surface as a panic rather
    /// than a declined rewrite.
    #[test]
    fn preserve_span_boundaries_never_panics(
        joined in mixed_width_text(),
        line_text in mixed_width_text(),
        start in 0usize..=32,
        end in 0usize..=32,
        width in 0usize..24,
    ) {
        let mut lines = vec![line_text];

        preserve_span_boundaries(
            &mut lines,
            &joined,
            OverlongSpan { range: start..end, pieces: vec!["replacement".to_owned()] },
            width,
        );
    }

    /// A range whose start splits a multibyte scalar cannot name a span, so the
    /// wrapped lines must come back untouched.
    ///
    /// The offset is derived from the text, which is what makes this reach the
    /// slicing guard on every sample instead of only on lucky ASCII input.
    #[test]
    fn preserve_span_boundaries_keeps_lines_for_mid_scalar_ranges(
        (joined, start) in joining_text_with_mid_scalar_offset(),
    ) {
        let mut lines = vec!["before after".to_owned()];
        let original = lines.clone();

        preserve_span_boundaries(
            &mut lines,
            &joined,
            OverlongSpan { range: start..joined.len(), pieces: vec!["replacement".to_owned()] },
            20,
        );

        prop_assert_eq!(lines, original);
    }

    /// A valid range whose text appears in no wrapped line names no reachable
    /// span, so the lines must be left alone.
    ///
    /// The `span_lookup_miss` path exists so a span the wrapper split
    /// differently degrades to ordinary greedy wrapping instead of duplicating
    /// or dropping prose.
    #[test]
    fn preserve_span_boundaries_keeps_lines_when_span_is_absent(
        (joined, start, end, lines) in absent_span_case(),
        width in 1usize..24,
    ) {
        let mut lines = lines;
        let original = lines.clone();

        preserve_span_boundaries(
            &mut lines,
            &joined,
            OverlongSpan { range: start..end, pieces: vec!["replacement".to_owned()] },
            width,
        );

        prop_assert_eq!(lines, original);
    }

    /// Reattaching prose must be answered for every edge and width, never
    /// panic.
    ///
    /// `reattach_prose` delegates to the greedy wrapper when the prose does not
    /// fit, so this also exercises the wrapper on the whitespace-heavy fragments
    /// that reach it from a real preserved seam.
    #[test]
    fn reattach_prose_never_panics(
        existing in mixed_width_text(),
        prose in mixed_width_text(),
        width in 1usize..24,
        is_leading in any::<bool>(),
    ) {
        let mut lines = vec![existing];
        let edge = if is_leading { ProseEdge::Leading } else { ProseEdge::Trailing };

        reattach_prose(&mut lines, &prose, width, edge);
    }

    /// Reattaching prose may reflow or move it, but must never lose or invent
    /// non-whitespace content.
    ///
    /// Comparing both sides with whitespace removed ignores the join space, the
    /// edge trim and the wrapper's own break handling, and states exactly the
    /// contract: the prose survives the reattachment. The leading edge places
    /// the prose before the preserved piece and the trailing edge after it.
    #[test]
    fn reattach_prose_preserves_non_whitespace_content(
        existing in prose_fragment(),
        prose in prose_fragment(),
        width in 1usize..24,
        is_leading in any::<bool>(),
    ) {
        let mut lines = vec![existing.clone()];
        let edge = if is_leading { ProseEdge::Leading } else { ProseEdge::Trailing };
        let expected = match edge {
            ProseEdge::Leading => format!("{prose}{existing}"),
            ProseEdge::Trailing => format!("{existing}{prose}"),
        };

        reattach_prose(&mut lines, &prose, width, edge);

        prop_assert_eq!(
            without_whitespace(&lines.concat()),
            without_whitespace(&expected),
            "reattaching {:?} at width {} must keep every visible character",
            prose,
            width,
        );
    }

    /// When the prose cannot sit beside the preserved piece, it is wrapped
    /// independently — and every line it produces must still respect the width
    /// limit.
    ///
    /// The no-panic and content properties say nothing about line length, so
    /// without this the independent-wrap path could emit an overlong line and
    /// still pass. The width is constructed so the join is declined while every
    /// token remains breakable, which is exactly the precondition under which
    /// the limit is a real obligation rather than an impossible one.
    #[test]
    fn reattach_prose_respects_width_when_it_wraps_independently(
        (existing, prose, width) in non_fitting_reattachment(),
        is_leading in any::<bool>(),
    ) {
        // Pin the precondition rather than trusting the generator: if the join
        // could have fitted, this property would silently be testing the exact
        // path it claims to exclude.
        prop_assert!(
            UnicodeWidthStr::width(existing.as_str())
                + UnicodeWidthStr::width(prose.as_str())
                > width,
            "{:?} plus {:?} fits in width {}, so the independent-wrap path is not reached",
            existing,
            prose,
            width,
        );

        let mut lines = vec![existing.clone()];
        let edge = if is_leading { ProseEdge::Leading } else { ProseEdge::Trailing };

        reattach_prose(&mut lines, &prose, width, edge);

        for line in &lines {
            prop_assert!(
                UnicodeWidthStr::width(line.as_str()) <= width,
                "reattaching {:?} at width {} produced overlong line {:?} in {:?}",
                prose,
                width,
                line,
                lines,
            );
        }
    }

    /// When the prose does fit beside the preserved piece, the join is exact:
    /// the edge line becomes the concatenation and nothing is reflowed.
    ///
    /// `width` is taken from the joined text itself, so the property exercises
    /// the fitting branch for every generated prefix rather than only for the
    /// wide widths that make it trivially reachable.
    #[test]
    fn reattach_prose_joins_exactly_when_the_result_fits(
        prefix in prose_fragment(),
        suffix in prose_fragment(),
    ) {
        let width = UnicodeWidthStr::width(format!("{prefix}{suffix}").as_str());
        let mut lines = vec![prefix.clone()];

        reattach_prose(&mut lines, &suffix, width, ProseEdge::Trailing);

        prop_assert_eq!(lines, vec![format!("{prefix}{suffix}")]);
    }
}

/// Prose that both `reattach_prose` and the greedy wrapper treat as content.
///
/// A fragment with no visible character is skipped by the helper by design, so
/// the content properties exclude it at generation time rather than asserting
/// about a path they never reach.
fn prose_fragment() -> impl Strategy<Value = String> {
    (
        select(vec!['a', 'é']),
        proptest::collection::vec(prop_oneof![Just('a'), Just('é'), Just(' ')], 0..9),
    )
        .prop_map(|(first, body)| std::iter::once(first).chain(body).collect())
}

/// A preserved piece and prose that is guaranteed *not* to fit beside it, so
/// the independent-wrap path is taken rather than the fitting join.
///
/// `width` is the wider of the two sides. That is exactly wide enough for the
/// preserved piece and for the prose on their own, so both remain wrappable and
/// no emitted line is overlong for reasons unrelated to the reattachment, yet
/// the concatenation always exceeds it, so `join_if_fits` must decline. Every
/// prose token is a single character, so no token is unbreakable at this width.
fn non_fitting_reattachment() -> impl Strategy<Value = (String, String, usize)> {
    (prose_fragment(), prose_fragment()).prop_map(|(existing, prose)| {
        let width = UnicodeWidthStr::width(existing.as_str())
            .max(UnicodeWidthStr::width(prose.as_str()))
            .max(1);
        (existing, prose, width)
    })
}

/// Drop whitespace so a property can compare visible content only.
fn without_whitespace(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}
