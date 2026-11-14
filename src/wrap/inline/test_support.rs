//! Test-only inline helpers that keep `inline.rs` under the file-size ceiling.

/// Attaches punctuation-only `token` text to the previous wrapped code line.
///
/// `lines` holds the previously rendered lines, `current` is the pending
/// current-line buffer, and `token` is the punctuation fragment under test.
/// The return value is `true` when the punctuation was appended to the last
/// line. This test-only helper assumes `token` is already isolated as a single
/// rendered token and never panics.
pub(crate) fn attach_punctuation_to_previous_line(
    lines: &mut [String],
    current: &str,
    token: &str,
) -> bool {
    if !current.is_empty() || !is_single_punctuation_mark(token) {
        return false;
    }

    let Some(last_line) = lines.last_mut() else {
        return false;
    };

    if last_line.trim_end().ends_with('`') {
        last_line.push_str(token);
        return true;
    }

    false
}

/// Reports whether `token` is exactly one of the trailing punctuation marks.
///
/// The length check is what rejects a multi-character run that merely contains
/// punctuation; without it a token like `",,"` would pass the `contains` test
/// and be appended as though it were a single mark.
fn is_single_punctuation_mark(token: &str) -> bool { token.len() == 1 && ".?!,:;".contains(token) }
