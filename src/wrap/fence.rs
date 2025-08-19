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
    in_code: &mut bool,
    fence_state: &mut Option<(char, usize)>,
) -> bool {
    if let Some((_f_indent, fence, _info)) = is_fence(line) {
        super::flush_paragraph(out, buf, indent, width);
        buf.clear();
        indent.clear();

        // Determine fence marker kind and length to manage open/close state.
        let marker_ch = fence.chars().next().unwrap_or('`');
        let marker_len = fence.chars().count();

        if *in_code {
            if let Some((open_ch, open_len)) = fence_state {
                // Only close if the marker matches and its length is >= opened length.
                if marker_ch == *open_ch && marker_len >= *open_len {
                    *in_code = false;
                    *fence_state = None;
                }
            }
            // Re-emit the fence line unmodified.
            out.push(line.to_string());
            return true;
        }

        // Open a new fenced block.
        *in_code = true;
        *fence_state = Some((marker_ch, marker_len));
        out.push(line.to_string());
        return true;
    }

    false
}
