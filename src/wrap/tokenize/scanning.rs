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

/// Check if a byte at the given index is preceded by an odd number of
/// backslashes.
///
/// An odd number of preceding backslashes means the byte is escaped.
pub(super) fn has_odd_backslash_escape_bytes(bytes: &[u8], mut idx: usize) -> bool {
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
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::alpha_prefix("abc123", 0, Some(char::is_alphabetic as fn(char) -> bool), None, Some(3), None)]
    #[case::numeric_suffix("abc123", 3, Some(char::is_numeric as fn(char) -> bool), None, Some("abc123".len()), None)]
    #[case::multibyte_scan("åßç123", 0, Some(char::is_alphabetic as fn(char) -> bool), None, Some("åßç123".find('1').unwrap_or("åßç123".len())), Some("åßç"))]
    #[case::collect_first_two("αβγδε", 0, None, Some("αβ".len()), None, Some("αβ"))]
    #[case::collect_middle("αβγδε", "αβ".len(), None, Some("αβ".len() + "γδ".len()), None, Some("γδ"))]
    fn scan_and_collect_cases(
        #[case] text: &str,
        #[case] start: usize,
        #[case] predicate: Option<fn(char) -> bool>,
        #[case] end: Option<usize>,
        #[case] expected_idx: Option<usize>,
        #[case] expected_str: Option<&str>,
    ) {
        if let Some(pred) = predicate {
            let idx = scan_while(text, start, pred);
            if let Some(expected) = expected_idx {
                assert_eq!(idx, expected);
            }
            if let Some(expected_slice) = expected_str {
                assert_eq!(&text[..idx], expected_slice);
            }
        } else if let Some(end_idx) = end {
            let collected = collect_range(text, start, end_idx);
            if let Some(expected_slice) = expected_str {
                assert_eq!(collected, expected_slice);
            }
        } else {
            panic!("Invalid test case configuration");
        }
    }
}
