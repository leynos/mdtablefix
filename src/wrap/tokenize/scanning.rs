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
/// When this predicate is true the bracket must be treated as literal text
/// rather than the start of an image token.
pub(super) fn bracket_follows_escaped_bang(bytes: &[u8], idx: usize) -> bool {
    if idx == 0 || bytes[idx - 1] != b'!' {
        return false;
    }
    has_odd_backslash_escape_bytes(bytes, idx - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_while_respects_predicate_boundaries() {
        let text = "abc123";
        assert_eq!(scan_while(text, 0, char::is_alphabetic), 3);
        assert_eq!(scan_while(text, 3, char::is_numeric), text.len());
    }

    #[test]
    fn scan_while_advances_over_multibyte_characters() {
        let text = "åßç123";
        let idx = scan_while(text, 0, char::is_alphabetic);
        assert_eq!(&text[..idx], "åßç");
    }

    #[test]
    fn collect_range_extracts_multibyte_segments() {
        let text = "αβγδε";
        let first_two = "αβ".len();
        let middle = first_two + "γδ".len();
        assert_eq!(collect_range(text, 0, first_two), "αβ");
        assert_eq!(collect_range(text, first_two, middle), "γδ");
    }
}
