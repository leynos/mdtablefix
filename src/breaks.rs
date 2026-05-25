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
/// standardised line of underscores. Fenced code blocks are ignored so
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
