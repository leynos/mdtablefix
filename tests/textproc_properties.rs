//! Property tests for shared text-processing helpers.

use mdtablefix::textproc::leading_indent;
use proptest::prelude::*;

proptest! {
    #[test]
    fn leading_indent_preserves_its_unicode_whitespace_contract(s: String) {
        let indent = leading_indent(&s);
        let remainder = &s[indent.len()..];

        prop_assert!(s.starts_with(indent));
        if let Some(character) = remainder.chars().next() {
            prop_assert!(!character.is_whitespace());
        }
        prop_assert_eq!(indent == s, s.chars().all(char::is_whitespace));
        prop_assert_eq!(leading_indent(indent), indent);
        prop_assert_eq!(leading_indent(remainder), "");
    }
}

// Bounded model checking is unsuitable here because this helper accepts
// unbounded UTF-8 strings; property testing exercises that input domain.
