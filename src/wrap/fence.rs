//! Fenced code block helpers.

use regex::Regex;

use super::{
    BlockquotePrefix,
    paragraph::{ParagraphState, ParagraphWriter},
};

pub(super) static FENCE_RE: std::sync::LazyLock<Regex> =
    // Capture: indent, fence run of 3+ backticks/tilde, and the full info string (incl. leading
    // spaces)
    lazy_regex!(
        r"^(\s*)(`{3,}|~{3,})([^\r\n]*)$",
        "wrapping fence delimiter and info string pattern should compile",
    );

/// Return fence components if the line starts a fenced code block.
///
/// The function captures:
/// - the leading indentation,
/// - the fence marker itself (three or more backticks or tildes),
/// - the full trailing “info string” (including any leading spaces and attributes).
///
/// # Examples
///
/// ```rust
/// use mdtablefix::wrap::is_fence;
/// assert_eq!(is_fence("```rust"), Some(("", "```", "rust")));
/// assert_eq!(is_fence("``` rust"), Some(("", "```", " rust")));
/// assert_eq!(is_fence("``` rust linenums"), Some(("", "```", " rust linenums")));
/// assert_eq!(is_fence("> > ```rust"), Some(("> > ", "```", "rust")));
/// assert!(is_fence("not a fence").is_none());
/// ```
#[doc(hidden)]
#[must_use]
#[rustfmt::skip]
pub fn is_fence(line: &str) -> Option<(&str, &str, &str)> {
    let context = FenceLine::parse(line);
    FENCE_RE.captures(context.inner).map(|cap| {
        let inner_indent = cap.get(1).map_or("", |m| m.as_str());
        let indent = &line[..context.prefix_len + inner_indent.len()];
        let fence  = cap.get(2).map_or("", |m| m.as_str());
        let info   = cap.get(3).map_or("", |m| m.as_str());
        (indent, fence, info)
    })
}

struct FenceLine<'a> {
    inner: &'a str,
    depth: usize,
    prefix_len: usize,
}

impl<'a> FenceLine<'a> {
    fn parse(line: &'a str) -> Self {
        BlockquotePrefix::parse(line).map_or(
            Self {
                inner: line,
                depth: 0,
                prefix_len: 0,
            },
            |prefix| Self {
                inner: prefix.inner(),
                depth: prefix.depth(),
                prefix_len: prefix.raw_prefix().len(),
            },
        )
    }
}
/// Handle a potential fence line, updating state and emitting the line when needed.
///
/// Returns `true` if the line was processed as a fence.
pub(crate) fn handle_fence_line(
    line: &str,
    inner_content: &str,
    depth: usize,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
    tracker: &mut FenceTracker,
) -> bool {
    if !tracker.observe(inner_content, depth) {
        return false;
    }

    writer.push_verbatim(state, line);
    true
}

/// Tracks Markdown fenced code block state across lines.
///
/// The tracker centralises fence matching logic so that callers share the
/// same semantics for opening and closing blocks.
///
/// # Examples
///
/// ```
/// use mdtablefix::wrap::FenceTracker;
///
/// let mut tracker = FenceTracker::new();
/// assert!(!tracker.in_fence(0));
/// assert!(tracker.observe("```rust", 0));
/// assert!(tracker.in_fence(0));
/// assert!(tracker.observe("```", 0));
/// assert!(!tracker.in_fence(0));
/// ```
#[derive(Default, Debug)]
pub struct FenceTracker {
    state: Option<(char, usize, usize)>,
}

impl FenceTracker {
    /// Create a new tracker with no active fence.
    #[must_use]
    pub fn new() -> Self { Self::default() }

    /// Update the tracker with a potential fence line.
    ///
    /// Returns `true` when the line is treated as a fence marker and updates
    /// the internal state accordingly.
    ///
    /// # Panics
    ///
    /// Panics when the fence regular expression yields an empty marker, which
    /// would indicate the regex is inconsistent with Markdown fence rules.
    #[must_use]
    pub fn observe(&mut self, line: &str, depth: usize) -> bool {
        if self
            .state
            .is_some_and(|(_open_ch, _open_len, open_depth)| depth < open_depth)
        {
            self.state = None;
        }

        let Some((_indent, fence, _info)) = is_fence(line) else {
            return false;
        };

        let mut chars = fence.chars();
        let marker_ch = chars.next().expect("FENCE_RE guarantees a non-empty fence");
        let marker_len = chars.count() + 1;

        match self.state {
            Some((open_ch, open_len, open_depth))
                if depth == open_depth && marker_ch == open_ch && marker_len >= open_len =>
            {
                self.state = None;
            }
            Some(_) => {}
            None => {
                self.state = Some((marker_ch, marker_len, depth));
            }
        }

        true
    }

    /// Update the tracker from a source line, including any blockquote prefix.
    ///
    /// This compatibility boundary is intended for processing stages that
    /// receive raw Markdown rather than the prefix-stripped input used by
    /// `wrap_text`.
    ///
    /// # Examples
    ///
    /// ```
    /// use mdtablefix::wrap::FenceTracker;
    ///
    /// let mut tracker = FenceTracker::new();
    /// assert!(tracker.observe_line("> > ```rust"));
    /// assert!(tracker.in_fence(2));
    /// ```
    #[must_use]
    pub fn observe_line(&mut self, line: &str) -> bool {
        let context = FenceLine::parse(line);
        self.observe(context.inner, context.depth)
    }

    /// Check whether the tracker is currently inside a fenced block.
    #[must_use]
    pub fn in_fence(&self, current_depth: usize) -> bool {
        self.state
            .is_some_and(|(_marker_ch, _marker_len, open_depth)| current_depth >= open_depth)
    }

    /// Check fence state at the blockquote depth represented by a source line.
    ///
    /// # Examples
    ///
    /// ```
    /// use mdtablefix::wrap::FenceTracker;
    ///
    /// let mut tracker = FenceTracker::new();
    /// assert!(tracker.observe_line("> ```"));
    /// assert!(tracker.in_fence_for_line("> code"));
    /// assert!(!tracker.in_fence_for_line("outside the quote"));
    /// ```
    #[must_use]
    pub fn in_fence_for_line(&self, line: &str) -> bool {
        self.in_fence(FenceLine::parse(line).depth)
    }
}

#[cfg(test)]
mod property_tests {
    //! Property coverage for the capture contract shared by wrapping fences.

    use proptest::prelude::*;

    use super::is_fence;

    proptest! {
        #[test]
        fn fence_captures_round_trip_generated_delimiters(
            indent in "[ \\t]{0,4}",
            blockquote_depth in 0_usize..=4,
            marker in prop_oneof![Just('`'), Just('~')],
            marker_length in 3_usize..=12,
            info in "[^\\r\\n]{0,40}",
        ) {
            let blockquote = "> ".repeat(blockquote_depth);
            let prefix = format!("{indent}{blockquote}");
            let delimiter = marker.to_string().repeat(marker_length);
            let line = format!("{prefix}{delimiter}{info}");
            let absorbed_marker_count = info.chars().take_while(|character| *character == marker).count();
            let expected_delimiter = marker.to_string().repeat(marker_length + absorbed_marker_count);
            let expected_info = &info[absorbed_marker_count..];

            let captures = is_fence(&line);

            prop_assert_eq!(
                captures,
                Some((prefix.as_str(), expected_delimiter.as_str(), expected_info)),
            );
            let (captured_prefix, captured_delimiter, captured_info) =
                captures.expect("generated fence should match");
            prop_assert_eq!(
                format!("{captured_prefix}{captured_delimiter}{captured_info}"),
                line,
            );
        }
    }
}
