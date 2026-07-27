//! Shared Markdown hard-break detection for paragraph flushing.
//!
//! Both the stable tail-reflow path and the spanning-code fallback need to know
//! whether a line already ends with a hard break before appending one. This
//! helper centralizes that parity check so the two paths stay consistent.

/// Returns the byte length of the trailing Markdown hard-break marker on `line`.
///
/// A hard break is either two trailing spaces (marker length 2) or an odd run
/// of trailing backslashes (marker length 1). Any other ending, including an
/// even backslash run or no marker at all, returns 0.
pub(super) fn trailing_hard_break_marker_len(line: &str) -> usize {
    if line.ends_with("  ") {
        return 2;
    }
    line.chars()
        .rev()
        .take_while(|character| *character == '\\')
        .count()
        % 2
}
