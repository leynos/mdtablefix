//! Fenced code block helpers.

use regex::Regex;

pub(super) static FENCE_RE: std::sync::LazyLock<Regex> =
    // Capture: indent, fence run of 3+ backticks/tilde, and the full info string (incl. leading
    // spaces)
    std::sync::LazyLock::new(|| {
        Regex::new(r"^(\s*)(`{3,}|~{3,})([^\r\n]*)$").expect("valid fence regex")
    });

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
/// assert!(is_fence("not a fence").is_none());
/// ```
#[doc(hidden)]
#[must_use]
#[rustfmt::skip]
pub fn is_fence(line: &str) -> Option<(&str, &str, &str)> {
    FENCE_RE.captures(line).map(|cap| {
        let indent = cap.get(1).map_or("", |m| m.as_str());
        let fence  = cap.get(2).map_or("", |m| m.as_str());
        let info   = cap.get(3).map_or("", |m| m.as_str());
        (indent, fence, info)
    })
}

/// Handle a potential fence line, updating state and emitting the line when needed.
///
/// Returns `true` if the line was processed as a fence.
pub(crate) fn handle_fence_line(
    out: &mut Vec<String>,
    buf: &mut Vec<(String, bool)>,
    indent: &mut String,
    width: usize,
    line: &str,
    tracker: &mut FenceTracker,
) -> bool {
    if !tracker.observe(line) {
        return false;
    }

    super::flush_paragraph(out, buf, indent, width);
    buf.clear();
    indent.clear();
    out.push(line.to_string());
    true
}

/// Tracks Markdown fenced code block state across lines.
///
/// The tracker centralises fence matching logic so that callers share the
/// same semantics for opening and closing blocks.
#[derive(Default)]
pub struct FenceTracker {
    state: Option<(char, usize)>,
}

impl FenceTracker {
    /// Create a new tracker with no active fence.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the tracker with a potential fence line.
    ///
    /// Returns `true` when the line is treated as a fence marker and updates
    /// the internal state accordingly.
    #[must_use]
    pub fn observe(&mut self, line: &str) -> bool {
        let Some((_indent, fence, _info)) = is_fence(line) else {
            return false;
        };

        let mut chars = fence.chars();
        let marker_ch = chars.next().unwrap_or('`');
        let marker_len = chars.count() + 1;

        match self.state {
            Some((open_ch, open_len)) if marker_ch == open_ch && marker_len >= open_len => {
                self.state = None;
            }
            Some(_) => {}
            None => {
                self.state = Some((marker_ch, marker_len));
            }
        }

        true
    }

    /// Check whether the tracker is currently inside a fenced block.
    #[must_use]
    pub fn in_fence(&self) -> bool {
        self.state.is_some()
    }
}
