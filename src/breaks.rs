//! Thematic break formatting utilities.

use std::borrow::Cow;

use regex::Regex;

use crate::wrap::FenceTracker;

pub const THEMATIC_BREAK_LEN: usize = 70;

pub(crate) static THEMATIC_BREAK_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"^[ ]{0,3}((?:[ \t]*\*){3,}|(?:[ \t]*-){3,}|(?:[ \t]*_){3,})[ \t]*$")
        .expect("valid thematic break regex")
});

static THEMATIC_BREAK_LINE: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| "_".repeat(THEMATIC_BREAK_LEN));

/// Normalize thematic breaks outside fenced code blocks.
///
/// Consecutive hyphens, asterisks or underscores are replaced with a
/// standardized line of underscores. Fenced code blocks are ignored so
/// that breaks within them remain untouched.
///
/// # Examples
///
/// ```
/// use std::borrow::Cow;
///
/// use mdtablefix::{THEMATIC_BREAK_LEN, format_breaks};
///
/// let lines = vec!["foo".to_string(), "***".to_string(), "bar".to_string()];
/// let out = format_breaks(&lines);
/// let break_line = "_".repeat(THEMATIC_BREAK_LEN);
/// assert_eq!(
///     out,
///     vec![
///         Cow::Borrowed("foo"),
///         Cow::Borrowed(break_line.as_str()),
///         Cow::Borrowed("bar"),
///     ]
/// );
/// ```
#[must_use]
pub fn format_breaks(lines: &[String]) -> Vec<Cow<'_, str>> {
    let mut out = Vec::with_capacity(lines.len());
    // Track fenced code blocks consistently while formatting breaks.
    let mut fences = FenceTracker::default();

    for line in lines {
        if fences.observe(line) {
            out.push(Cow::Borrowed(line.as_str()));
            continue;
        }

        if !fences.in_fence() && THEMATIC_BREAK_RE.is_match(line.trim_end()) {
            out.push(Cow::Borrowed(THEMATIC_BREAK_LINE.as_str()));
        } else {
            out.push(Cow::Borrowed(line.as_str()));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    macro_rules! assert_borrowed_value {
        ($line:expr, $expected:expr $(,)?) => {
            match &$line {
                Cow::Borrowed(value) => assert_eq!(*value, $expected),
                Cow::Owned(value) => panic!("expected borrowed value, got owned {value:?}"),
            }
        };
    }

    #[test]
    fn basic_formatting() {
        let input = vec!["foo", "***", "bar"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let output = format_breaks(&input);

        assert_borrowed_value!(output[0], "foo");
        assert_borrowed_value!(output[1], THEMATIC_BREAK_LINE.as_str());
        assert_borrowed_value!(output[2], "bar");
    }

    #[test]
    fn ignores_fenced_code() {
        let input = vec!["```", "---", "```"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let output = format_breaks(&input);

        assert_borrowed_value!(output[0], "```");
        assert_borrowed_value!(output[1], "---");
        assert_borrowed_value!(output[2], "```");
    }
}

#[cfg(test)]
mod prop_tests {
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
            prefix in prop::collection::vec(non_thematic_line(), 0..8),
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
                    prop_assert!(std::ptr::eq(*value, lines[fence_break_idx].as_str()));
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
        any::<String>().prop_filter("line must not match thematic break regex", |line| {
            !THEMATIC_BREAK_RE.is_match(line.trim_end())
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
}
