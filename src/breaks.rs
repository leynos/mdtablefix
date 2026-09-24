//! Thematic break formatting utilities.

use std::borrow::Cow;

use regex::Regex;

use crate::wrap::FenceTracker;

/// Number of underscores in the canonical replacement for a thematic break.
///
/// # Examples
///
/// ```
/// use mdtablefix::THEMATIC_BREAK_LEN;
///
/// let canonical_break = "_".repeat(THEMATIC_BREAK_LEN);
/// assert_eq!(canonical_break.len(), 70);
/// ```
pub const THEMATIC_BREAK_LEN: usize = 70;

/// Recognizes a Markdown thematic break while allowing up to three columns of indentation.
///
/// The expression accepts spaces and tabs between markers because those forms are valid thematic
/// breaks, while the formatter supplies one canonical replacement line.
pub(crate) static THEMATIC_BREAK_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^[ ]{0,3}((?:[ \t]*\*){3,}|(?:[ \t]*-){3,}|(?:[ \t]*_){3,})[ \t]*$",
    "thematic break pattern should compile",
);

/// Shared replacement line so every thematic break can be returned without allocation.
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
        let fence = fences.observe_source_line(line);
        if fence.is_fence_marker {
            out.push(Cow::Borrowed(line.as_str()));
            continue;
        }

        if !fence.is_in_fence && THEMATIC_BREAK_RE.is_match(line.trim_end()) {
            out.push(Cow::Borrowed(THEMATIC_BREAK_LINE.as_str()));
        } else {
            out.push(Cow::Borrowed(line.as_str()));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    //! Unit tests for thematic-break formatting.

    use std::{
        borrow::Cow,
        sync::{Arc, Barrier},
        thread,
    };

    use super::*;

    macro_rules! assert_borrowed_value {
        ($line:expr, $expected:expr $(,)?) => {
            match $line {
                Cow::Borrowed(value) => assert_eq!(*value, $expected),
                Cow::Owned(value) => panic!("expected borrowed value, got owned {value:?}"),
            }
        };
    }

    #[test]
    fn basic_formatting() {
        let input = ["foo", "***", "bar"].map(str::to_owned);
        let output = format_breaks(&input);
        let [first, middle, last] = output.as_slice() else {
            panic!("expected three formatted lines, got {}", output.len());
        };

        assert_borrowed_value!(first, "foo");
        assert_borrowed_value!(middle, THEMATIC_BREAK_LINE.as_str());
        assert_borrowed_value!(last, "bar");
    }

    #[test]
    fn ignores_fenced_code() {
        let input = ["```", "---", "```"].map(str::to_owned);
        let output = format_breaks(&input);
        let [opening, middle, closing] = output.as_slice() else {
            panic!("expected three formatted lines, got {}", output.len());
        };

        assert_borrowed_value!(opening, "```");
        assert_borrowed_value!(middle, "---");
        assert_borrowed_value!(closing, "```");
    }

    #[test]
    fn lazylock_initialisation_is_race_safe() {
        const THREADS: usize = 16;

        // Document the application's reliance on the stdlib race-safety guarantee.
        let barrier = Arc::new(Barrier::new(THREADS));
        let handles = (0..THREADS)
            .map(|_| {
                let start_barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let input = vec!["---".to_owned()];
                    start_barrier.wait();

                    let output = format_breaks(&input);
                    let [Cow::Borrowed(value)] = output.as_slice() else {
                        panic!("expected one borrowed break line, got {output:?}");
                    };
                    assert_eq!(*value, THEMATIC_BREAK_LINE.as_str());
                    assert_eq!(value.len(), THEMATIC_BREAK_LEN);
                    assert!(std::ptr::eq(*value, THEMATIC_BREAK_LINE.as_str()));
                    value.as_ptr() as usize
                })
            })
            .collect::<Vec<_>>();

        let pointers = handles
            .into_iter()
            .map(|handle| handle.join().expect("thread must complete"))
            .collect::<Vec<_>>();

        assert!(
            pointers
                .iter()
                .all(|pointer| *pointer == THEMATIC_BREAK_LINE.as_ptr() as usize)
        );
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

            for (input, formatted_line) in lines.iter().zip(output) {
                match formatted_line {
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
            match output.as_slice() {
                [Cow::Borrowed(value)] => {
                    prop_assert_eq!(*value, THEMATIC_BREAK_LINE.as_str());
                    prop_assert_eq!(value.len(), THEMATIC_BREAK_LEN);
                    prop_assert!(std::ptr::eq(*value, THEMATIC_BREAK_LINE.as_str()));
                }
                [Cow::Owned(value)] => {
                    prop_assert!(false, "expected borrowed break line, got owned {value:?}");
                }
                _ => prop_assert!(false, "expected one output line, got {}", output.len()),
            }
        }

        #[test]
        fn fenced_thematic_breaks_are_not_normalised(
            fencer in prop_oneof![Just("```".to_owned()), Just("~~~".to_owned())],
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
            match lines.iter().zip(&output).nth(fence_break_idx) {
                Some((source, Cow::Borrowed(value))) => {
                    prop_assert_eq!(*value, break_line.as_str());
                    prop_assert!(
                        std::ptr::eq(*value, source.as_str()),
                        "fenced break line must borrow from input, not from static"
                    );
                }
                Some((_, Cow::Owned(value))) => {
                    prop_assert!(
                        false,
                        "expected borrowed input line inside fence, got owned {value:?}"
                    );
                }
                None => prop_assert!(false, "missing fenced break line at {fence_break_idx}"),
            }
        }
    }

    fn non_thematic_line() -> impl Strategy<Value = String> {
        any::<String>().prop_filter("line must not match thematic break regex", |line| {
            !THEMATIC_BREAK_RE.is_match(line.trim_end())
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
}
