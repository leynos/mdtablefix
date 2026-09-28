//! Property-based tests for thematic break formatting and `Cow`
//! allocation semantics in [`format_breaks`].
//!
//! Uses the `non_thematic_line` and `thematic_break_line` strategies to
//! exercise the `Cow` allocation invariants: every output line preserves
//! input length, non-thematic lines stay borrowed from the input, and
//! thematic-break lines stay borrowed from the shared static.

use std::borrow::Cow;

use proptest::prelude::*;

use super::*;

proptest! {
    #[test]
    fn output_length_matches_input_length(lines in prop::collection::vec(any::<String>(), 0..128)) {
        let output = format_breaks(&lines);

        prop_assert_eq!(output.len(), lines.len());
    }

    #[test]
    fn non_thematic_lines_are_borrowed_from_input(
        lines in prop::collection::vec(non_thematic_line(), 0..128),
    ) {
        let output = format_breaks(&lines);

        for (input, output) in lines.iter().zip(output) {
            match output {
                Cow::Borrowed(value) => {
                    prop_assert_eq!(value, input.as_str());
                    prop_assert!(std::ptr::eq(value, input.as_str()));
                }
                Cow::Owned(value) => {
                    prop_assert!(false, "expected borrowed input line, got owned {value:?}");
                }
            }
        }
    }

    #[test]
    fn thematic_break_lines_are_borrowed_from_static(line in thematic_break_line()) {
        let input = vec![line];
        let output = format_breaks(&input);

        prop_assert_eq!(output.len(), 1);
        match &output[0] {
            Cow::Borrowed(value) => {
                prop_assert_eq!(*value, THEMATIC_BREAK_LINE.as_str());
                prop_assert_eq!(value.len(), THEMATIC_BREAK_LEN);
                prop_assert!(std::ptr::eq(*value, THEMATIC_BREAK_LINE.as_str()));
            }
            Cow::Owned(value) => {
                prop_assert!(false, "expected borrowed break line, got owned {value:?}");
            }
        }
    }

    #[test]
    fn fenced_thematic_breaks_are_not_normalised(
        fencer in prop_oneof![Just("```".to_string()), Just("~~~".to_string())],
        break_line in thematic_break_line(),
        prefix in prop::collection::vec(non_fence_line(), 0..8),
        suffix in prop::collection::vec(non_thematic_line(), 0..8),
    ) {
        let mut lines: Vec<String> = prefix.clone();
        lines.push(fencer.clone());
        lines.push(break_line.clone());
        lines.push(fencer.clone());
        lines.extend(suffix.clone());

        let output = format_breaks(&lines);
        prop_assert_eq!(output.len(), lines.len());

        let fence_break_idx = prefix.len() + 1;
        match &output[fence_break_idx] {
            Cow::Borrowed(value) => {
                prop_assert_eq!(*value, break_line.as_str());
                prop_assert!(
                    std::ptr::eq(*value, lines[fence_break_idx].as_str()),
                    "fenced break line must borrow from input, not from static"
                );
            }
            Cow::Owned(value) => {
                prop_assert!(
                    false,
                    "expected borrowed input line inside fence, got owned {value:?}"
                );
            }
        }
    }
}

fn non_thematic_line() -> impl Strategy<Value = String> {
    any::<String>().prop_filter("line must not classify as a thematic break", |line| {
        !is_canonical_break_line(line, &ClassifyCtx::default())
    })
}

fn non_fence_line() -> impl Strategy<Value = String> {
    non_thematic_line().prop_filter("line must not be a fence", |line| {
        crate::wrap::is_fence(line).is_none()
    })
}

fn thematic_break_line() -> impl Strategy<Value = String> {
    (
        0usize..=3,
        prop_oneof![Just('*'), Just('-'), Just('_')],
        3usize..80,
        prop::collection::vec(prop_oneof![Just(' '), Just('\t')], 0..8),
    )
        .prop_map(|(indent, marker, count, trailing)| {
            let mut line = String::with_capacity(indent + count + trailing.len());
            line.push_str(&" ".repeat(indent));
            line.push_str(&marker.to_string().repeat(count));
            line.extend(trailing);
            line
        })
}
