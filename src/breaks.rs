//! Thematic break formatting utilities.

use std::borrow::Cow;

use crate::{
    classify::{
        ClassifiedLine,
        ClassifyCtx,
        LineClass,
        ListContinuationState,
        classify_line_with_body,
        is_canonical_break_line,
        quote_depth,
        structural_content_indent,
    },
    wrap::{FenceTracker, LinkReferenceMatcher, classify_residual_block},
};

pub const THEMATIC_BREAK_LEN: usize = 70;

/// Shared replacement line so every thematic break can be returned without allocation.
static THEMATIC_BREAK_LINE: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| "_".repeat(THEMATIC_BREAK_LEN));

/// Context retained between adjacent lines in the break pass.
#[derive(Default)]
struct BreakLineState {
    /// Prior class, quote depth, and list content column, when available.
    previous: Option<(LineClass, usize, Option<usize>)>,
    /// Active list indentation tracked for Setext decisions.
    lists: ListContinuationState,
}

impl BreakLineState {
    /// Forgets context at a fenced-code boundary.
    fn reset(&mut self) {
        self.previous = None;
        self.lists.reset();
    }

    /// Selects the classifier context for the next source line.
    fn context(&self, line: &str, first_pass: &ClassifiedLine<'_>, depth: usize) -> ClassifyCtx {
        if self.continues_paragraph(line, first_pass, depth) {
            ClassifyCtx::following(LineClass::ParagraphText, true)
        } else {
            ClassifyCtx::default()
        }
    }

    /// Checks whether the preceding text can supply a Setext prefix.
    fn continues_paragraph(
        &self,
        line: &str,
        classified: &ClassifiedLine<'_>,
        depth: usize,
    ) -> bool {
        self.previous
            .is_some_and(|(class, old_depth, continuation_indent)| {
                (class == LineClass::ParagraphText
                    || (class == LineClass::ListItem && continuation_indent.is_some()))
                    && old_depth == depth
                    && continuation_indent.is_none_or(|indent| {
                        structural_content_indent(line, classified.body) >= indent
                    })
            })
    }

    /// Records a structural line for the next classifier decision.
    fn observe(
        &mut self,
        line: &str,
        classified: &ClassifiedLine<'_>,
        depth: usize,
        link_matcher: LinkReferenceMatcher,
    ) {
        let is_residual_block = classified.class == LineClass::ParagraphText
            && classify_residual_block(classified.body.trim(), link_matcher).is_some();
        let continuation_indent = if is_residual_block {
            self.lists.reset();
            None
        } else {
            self.lists.observe(line, classified)
        };
        self.previous = if classified.class == LineClass::Blank || is_residual_block {
            None
        } else {
            Some((classified.class, depth, continuation_indent))
        };
    }
}

/// Returns the canonical thematic break emitted by [`format_breaks`].
#[must_use]
pub(crate) fn canonical_break() -> &'static str { THEMATIC_BREAK_LINE.as_str() }

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
    let link_matcher = LinkReferenceMatcher::production();
    let mut state = BreakLineState::default();

    for line in lines {
        let fence = fences.observe_source_line(line);
        if fence.is_fence_marker || fence.is_in_fence {
            state.reset();
            out.push(Cow::Borrowed(line.as_str()));
            continue;
        }

        let first_pass = classify_line_with_body(line, &ClassifyCtx::default());
        let prefix_len = line.len() - first_pass.body.len();
        let prefix = &line[..prefix_len];
        let depth = quote_depth(prefix);
        let context = state.context(line, &first_pass, depth);
        let classified = if context == ClassifyCtx::default() {
            first_pass
        } else {
            classify_line_with_body(line, &context)
        };
        state.observe(line, &classified, depth, link_matcher);

        if is_canonical_break_line(line, &context) {
            out.push(canonicalized_break(prefix));
        } else {
            out.push(Cow::Borrowed(line.as_str()));
        }
    }

    out
}

/// Retains a quote prefix when emitting the shared canonical break line.
fn canonicalized_break(prefix: &str) -> Cow<'static, str> {
    if prefix.contains('>') {
        Cow::Owned(format!("{prefix}{}", canonical_break()))
    } else {
        Cow::Borrowed(canonical_break())
    }
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

    #[test]
    fn lazylock_initialisation_is_race_safe() {
        const THREADS: usize = 16;

        // Document the application's reliance on the stdlib race-safety guarantee.
        let barrier = Arc::new(Barrier::new(THREADS));
        let handles = (0..THREADS)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let input = vec!["---".to_string()];
                    barrier.wait();

                    let output = format_breaks(&input);
                    match &output[0] {
                        Cow::Borrowed(value) => {
                            assert_eq!(*value, THEMATIC_BREAK_LINE.as_str());
                            assert_eq!(value.len(), THEMATIC_BREAK_LEN);
                            assert!(std::ptr::eq(*value, THEMATIC_BREAK_LINE.as_str()));
                            value.as_ptr() as usize
                        }
                        Cow::Owned(value) => {
                            panic!("expected borrowed break line, got owned {value:?}");
                        }
                    }
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
}
