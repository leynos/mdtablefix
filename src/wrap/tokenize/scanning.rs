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

pub fn has_unclosed_code_span(text: &str) -> bool {
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
        let mut search = fence_end;
        let mut found_close = false;

        while search < text.len() {
            let candidate_end = scan_while(text, search, |candidate| candidate == '`');
            if candidate_end > search
                && candidate_end - search == fence_len
                && !has_odd_backslash_escape_bytes(bytes, search)
            {
                found_close = true;
                index = candidate_end;
                break;
            }

            if let Some(next) = text[search..].chars().next() {
                search += next.len_utf8();
            } else {
                break;
            }
        }

        if !found_close {
            return true;
        }
    }
    false
}
mod tests {
    use rstest::rstest;

    use super::*;

    struct ScanCollectCase {
        text: &'static str,
        start: usize,
        predicate: Option<fn(char) -> bool>,
        end: Option<usize>,
        expected_idx: Option<usize>,
        expected_str: Option<&'static str>,
    }

    #[rstest]
    #[case::alpha_prefix(ScanCollectCase { text: "abc123", start: 0, predicate: Some(char::is_alphabetic as fn(char) -> bool), end: None, expected_idx: Some(3), expected_str: None })]
    #[case::numeric_suffix(ScanCollectCase { text: "abc123", start: 3, predicate: Some(char::is_numeric as fn(char) -> bool), end: None, expected_idx: Some("abc123".len()), expected_str: None })]
    #[case::multibyte_scan(ScanCollectCase { text: "åßç123", start: 0, predicate: Some(char::is_alphabetic as fn(char) -> bool), end: None, expected_idx: Some("åßç123".find('1').unwrap_or("åßç123".len())), expected_str: Some("åßç") })]
    #[case::collect_first_two(ScanCollectCase { text: "αβγδε", start: 0, predicate: None, end: Some("αβ".len()), expected_idx: None, expected_str: Some("αβ") })]
    #[case::collect_middle(ScanCollectCase { text: "αβγδε", start: "αβ".len(), predicate: None, end: Some("αβ".len() + "γδ".len()), expected_idx: None, expected_str: Some("γδ") })]
    fn scan_and_collect_cases(#[case] case: ScanCollectCase) {
        if let Some(pred) = case.predicate {
            let idx = scan_while(case.text, case.start, pred);
            if let Some(expected) = case.expected_idx {
                assert_eq!(idx, expected);
            }
            if let Some(expected_slice) = case.expected_str {
                assert_eq!(&case.text[..idx], expected_slice);
            }
        } else if let Some(end_idx) = case.end {
            let collected = collect_range(case.text, case.start, end_idx);
            if let Some(expected_slice) = case.expected_str {
                assert_eq!(collected, expected_slice);
            }
        } else {
            panic!("Invalid test case configuration");
        }
    }

    #[rstest]
    #[case("`VarGuard`s alive", "`VarGuard`".len(), "`VarGuard`s".len())]
    #[case("`class`'s field", "`class`".len(), "`class`'s".len())]
    #[case("`code`-style name", "`code`".len(), "`code`-style".len())]
    #[case("`code`-2 next", "`code`".len(), "`code`".len())]
    #[case("`code`.", "`code`".len(), "`code`".len())]
    #[case("`code`**", "`code`".len(), "`code`".len())]
    #[case("`code`'2 next", "`code`".len(), "`code`".len())]
    fn scan_code_suffix_end_cases(
        #[case] text: &str,
        #[case] start: usize,
        #[case] expected: usize,
    ) {
        assert_eq!(scan_code_suffix_end(text, start), expected);
    }

    use proptest::prelude::*;

    proptest! {
        /// scan_code_suffix_end must always return a byte index within the string.
        #[test]
        fn scan_code_suffix_end_result_in_bounds(
            text in "\\PC*",            // any non-control Unicode string
            start in 0usize..=128usize,
        ) {
            let start = start.min(text.len());
            let start = text.floor_char_boundary(start);
            let result = scan_code_suffix_end(&text, start);
            prop_assert!(result >= start, "result must be >= start");
            prop_assert!(result <= text.len(), "result must be <= text.len()");
        }

        /// scan_code_suffix_end must not advance past a non-suffix character.
        #[test]
        fn scan_code_suffix_end_no_advance_on_whitespace_start(
            suffix in " [a-z]{0,8}",
        ) {
            // A suffix beginning with whitespace should not be absorbed.
            let text = format!("`code`{suffix}");
            let start = 6usize; // index immediately after the closing backtick
            let result = scan_code_suffix_end(&text, start);
            prop_assert_eq!(result, start, "whitespace-led suffix must not be absorbed");
        }
    }
}
