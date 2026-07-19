//! Byte-level scanning helpers used by the tokenizer.
//!
//! Keeping these routines in their own module prevents the main tokenizer
//! module from growing too large while giving the low-level helpers a
//! dedicated place for documentation and unit tests.

/// Advance `idx` while the predicate evaluates to `true`.
///
/// Returns the byte index of the first character for which `cond` fails.
/// This small helper keeps the scanning loops concise and avoids
/// materialising the source as a char buffer.
///
/// # Examples
///
/// ```rust,ignore
/// let text = "abc123";
/// let end = scan_while(text, 0, char::is_alphabetic);
/// assert_eq!(end, 3);
/// ```
pub(super) fn scan_while<F>(text: &str, start: usize, mut cond: F) -> usize
where
    F: FnMut(char) -> bool,
{
    let mut idx = start;
    for ch in text[start..].chars() {
        if !cond(ch) {
            break;
        }
        idx += ch.len_utf8();
    }
    idx
}

/// Advance past an inflectional suffix glued to a closing inline-code fence.
///
/// Recognises alphabetic affixes (`s`, `ed`, `ing`), possessives (`'s`), and
/// hyphenated compounds (`-style`). Returns `start` when no suffix is present.
///
/// # Examples
///
/// ```rust,ignore
/// let text = "`VarGuard`s alive";
/// let close = 0 + "`VarGuard`".len();
/// assert_eq!(scan_code_suffix_end(text, close), close + 1);
/// ```
pub(super) fn scan_code_suffix_end(text: &str, start: usize) -> usize {
    if start >= text.len() {
        return start;
    }

    let rest = &text[start..];
    if rest.starts_with('-') {
        let first = rest.chars().nth(1);
        if first.is_some_and(char::is_alphabetic) {
            let after_hyphen = scan_while(text, start + 1, |ch| ch.is_alphabetic() || ch == '-');
            if after_hyphen > start + 1 {
                return after_hyphen;
            }
        }
    }

    if rest.starts_with('\'') {
        let after_apostrophe = scan_while(text, start + 1, char::is_alphabetic);
        if after_apostrophe > start + 1 {
            return after_apostrophe;
        }
    }

    scan_while(text, start, char::is_alphabetic)
}

/// Collect a range of characters into a [`String`].
///
/// # Examples
///
/// ```rust,ignore
/// let text = "abc";
/// assert_eq!(collect_range(text, 0, 2), "ab");
/// ```
pub(super) fn collect_range(text: &str, start: usize, end: usize) -> String {
    text[start..end].to_string()
}

pub(super) const BACKSLASH_BYTE: u8 = b'\\';
const BACKTICK_BYTE: u8 = b'`';

/// Returns the end index when `search` starts an exact backtick fence run.
fn closing_fence_end(bytes: &[u8], text: &str, search: usize, fence_len: usize) -> Option<usize> {
    if search >= text.len() {
        return None;
    }

    let ch = text[search..].chars().next()?;
    if ch != '`' {
        return None;
    }

    if search > 0 && bytes[search - 1] == BACKTICK_BYTE {
        return None;
    }

    let candidate_end = scan_while(text, search, |candidate| candidate == '`');
    if candidate_end - search != fence_len {
        return None;
    }

    if candidate_end < bytes.len() && bytes[candidate_end] == BACKTICK_BYTE {
        return None;
    }

    Some(candidate_end)
}

/// Returns the fence length when `text` begins with a backtick run.
pub(crate) fn opening_fence_run_len(bytes: &[u8], text: &str) -> Option<usize> {
    if text.is_empty() {
        return None;
    }

    let ch = text.chars().next()?;
    if ch != '`' || has_odd_backslash_escape_bytes(bytes, 0) {
        return None;
    }

    let run_end = scan_while(text, 0, |candidate| candidate == '`');
    let fence_len = run_end;
    if run_end < bytes.len() && bytes[run_end] == BACKTICK_BYTE {
        return None;
    }

    Some(fence_len)
}

pub(crate) fn parse_open_code_span(text: &str) -> Option<(usize, &str)> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < text.len() {
        let Some(ch) = text[index..].chars().next() else {
            break;
        };
        if ch != '`' || has_odd_backslash_escape_bytes(bytes, index) {
            index += ch.len_utf8();
            continue;
        }

        let fence_start = index;
        let fence_end = scan_while(text, index, |candidate| candidate == '`');
        let fence_len = fence_end - fence_start;
        if let Some(candidate_end) = position_after_close(text, fence_end, fence_len) {
            index = candidate_end;
            continue;
        }

        return Some((fence_len, &text[fence_end..]));
    }
    None
}
/// Check if a byte at the given index is preceded by an odd number of
/// backslashes.
///
/// An odd number of preceding backslashes means the byte is escaped.
pub(crate) fn has_odd_backslash_escape_bytes(bytes: &[u8], mut idx: usize) -> bool {
    let mut count = 0;
    while idx > 0 {
        idx -= 1;
        if bytes[idx] == BACKSLASH_BYTE {
            count += 1;
        } else {
            break;
        }
    }
    count % 2 == 1
}

/// Check whether a `[` at `idx` follows an escaped `!` (i.e. "\\![").
///
/// Returns `false` when `idx == 0` because there is no preceding character, so
/// the bracket cannot follow an escaped bang at the very beginning of the
/// string. Otherwise verifies the previous byte is `b'!'` and delegates to
/// [`has_odd_backslash_escape_bytes`] to confirm the bang was escaped by an odd
/// number of backslashes.
pub(super) fn bracket_follows_escaped_bang(bytes: &[u8], idx: usize) -> bool {
    if idx == 0 || bytes[idx - 1] != b'!' {
        return false;
    }
    has_odd_backslash_escape_bytes(bytes, idx - 1)
}

#[cfg(test)]
pub(crate) fn has_unclosed_code_span(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < text.len() {
        let Some(ch) = text[index..].chars().next() else {
            break;
        };
        if ch != '`' || has_odd_backslash_escape_bytes(bytes, index) {
            index += ch.len_utf8();
            continue;
        }

        let fence_start = index;
        let fence_end = scan_while(text, index, |candidate| candidate == '`');
        let fence_len = fence_end - fence_start;
        if let Some(candidate_end) = position_after_close(text, fence_end, fence_len) {
            index = candidate_end;
            continue;
        }

        return true;
    }
    false
}

/// Returns the byte position just past the first matching closing fence run.
///
/// Walks `text` looking for an unescaped backtick run of exactly
/// `fence_len` characters. When one is found, returns the byte index of the
/// character following the closing run. Returns `None` when no matching run
/// exists, or when `fence_len` is zero (since a zero-length fence cannot
/// match any real backtick run).
///
/// This helper is used by `split_reopen_span` so it can skip past the close
/// of a pre-existing span before searching the remainder for a new opener.
pub(crate) fn position_after_close(
    text: &str,
    search_start: usize,
    fence_len: usize,
) -> Option<usize> {
    if fence_len == 0 {
        return None;
    }

    let bytes = text.as_bytes();
    let mut index = search_start;
    let mut escaped_candidate_end = None;

    let has_matching_closing_fence = |search_from: usize| -> bool {
        let mut close_index = search_from;

        while close_index < text.len() {
            let Some(ch) = text[close_index..].chars().next() else {
                break;
            };

            if ch == '`'
                && let Some(_end) = closing_fence_end(bytes, text, close_index, fence_len)
            {
                return true;
            }

            close_index += ch.len_utf8();
        }

        false
    };

    while index < text.len() {
        let ch = text[index..].chars().next()?;
        if ch == '`'
            && let Some(end) = closing_fence_end(bytes, text, index, fence_len)
        {
            if has_odd_backslash_escape_bytes(bytes, index) {
                escaped_candidate_end = Some(end);
                index = end;
                continue;
            }
            if let Some(escaped_end) = escaped_candidate_end
                && has_matching_closing_fence(end)
            {
                return Some(escaped_end);
            }
            return Some(end);
        }
        index += ch.len_utf8();
    }
    escaped_candidate_end
}

pub(crate) fn scan_continuation_span_state(continuation: &str, fence_len: usize) -> Option<usize> {
    let bytes = continuation.as_bytes();
    let mut index = 0;
    let mut current_fence: Option<usize> = Some(fence_len);

    while index < continuation.len() {
        let Some(ch) = continuation[index..].chars().next() else {
            break;
        };

        if ch == '`' {
            if let Some(open_len) = current_fence {
                if let Some(end) = closing_fence_end(bytes, continuation, index, open_len) {
                    current_fence = None;
                    index = end;
                    continue;
                }
            } else {
                let fence_end = scan_while(continuation, index, |c| c == '`');
                let run = fence_end - index;
                let isolated = fence_end >= bytes.len() || bytes[fence_end] != BACKTICK_BYTE;
                if isolated && !has_odd_backslash_escape_bytes(bytes, index) {
                    current_fence = Some(run);
                    index = fence_end;
                    continue;
                }
            }
        }

        index += ch.len_utf8();
    }

    current_fence
}
/// Returns whether `continuation` begins with a closing fence for the open span
/// in `existing`.
#[cfg(test)]
#[must_use]
pub(crate) fn continuation_begins_with_closing_fence(existing: &str, continuation: &str) -> bool {
    let Some((open_fence_len, _content)) = parse_open_code_span(existing) else {
        return false;
    };

    let Some(run_len) = opening_fence_run_len(continuation.as_bytes(), continuation) else {
        return false;
    };

    open_fence_len == run_len
}
#[cfg(test)]
#[path = "scanning_tests.rs"]
mod tests;
