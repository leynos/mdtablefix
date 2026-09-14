//! Shared Markdown hard-break detection for paragraph flushing.
//!
//! Both the stable tail-reflow path and the spanning-code fallback need to know
//! whether a line already ends with a hard break before appending one. This
//! helper centralizes that parity check so the two paths stay consistent.

use tracing::trace;

/// Returns the byte length of the trailing Markdown hard-break marker on `line`.
///
/// A hard break is either two trailing spaces (marker length 2) or an odd run
/// of trailing backslashes (marker length 1). Any other ending, including an
/// even backslash run or no marker at all, returns 0.
pub(super) fn trailing_hard_break_marker_len(line: &str) -> usize {
    if line.ends_with("  ") {
        trace!(
            line_len = line.len(),
            marker_len = 2,
            marker = "two_spaces",
            "measuring the trailing hard break marker"
        );
        return 2;
    }
    let marker_len = line
        .chars()
        .rev()
        .take_while(|character| *character == '\\')
        .count()
        % 2;
    let marker = if marker_len == 1 { "backslash" } else { "none" };
    trace!(
        line_len = line.len(),
        marker_len, marker, "measuring the trailing hard break marker"
    );
    marker_len
}
