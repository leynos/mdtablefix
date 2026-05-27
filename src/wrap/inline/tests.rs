//! Unit tests for inline fragment classification helpers.
//!
//! This module covers the fragment classification rules that feed the inline
//! wrapping pipeline. It verifies how `InlineFragment` identifies whitespace,
//! inline code, links, footnote references, and plain text, and it checks the
//! recorded display width that the wrapper uses when measuring candidate
//! breaks.

use rstest::rstest;
use unicode_width::UnicodeWidthStr;

use super::fragment::{FragmentKind, InlineFragment, width_as_f64};

#[test]
fn inline_fragment_new_marks_spaces_as_whitespace() {
    let fragment = InlineFragment::new("   ".into());
    assert_eq!(fragment.kind, FragmentKind::Whitespace);
}

#[test]
fn inline_fragment_new_marks_backtick_spans_as_inline_code() {
    let fragment = InlineFragment::new("`code`".into());
    assert_eq!(fragment.kind, FragmentKind::InlineCode);
}

#[test]
fn inline_fragment_new_marks_markdown_links_as_links() {
    let fragment = InlineFragment::new("[text](url)".into());
    assert_eq!(fragment.kind, FragmentKind::Link);
}

#[rstest]
#[case("[^label]")]
#[case("word.[^label]")]
fn inline_fragment_new_marks_footnote_refs_as_footnote_refs(#[case] input: &str) {
    let fragment = InlineFragment::new(input.into());
    assert_eq!(fragment.kind, FragmentKind::FootnoteRef);
    assert!(fragment.is_atomic());
}

#[test]
fn inline_fragment_new_marks_plain_words_as_plain() {
    let fragment = InlineFragment::new("word".into());
    assert_eq!(fragment.kind, FragmentKind::Plain);
}

#[test]
fn inline_fragment_new_records_unicode_display_width() {
    let text = "表🙂";
    let fragment = InlineFragment::new(text.into());
    assert_eq!(fragment.width, UnicodeWidthStr::width(text));
}

#[rstest]
#[case(0, 0.0)]
#[case(42, 42.0)]
#[case(u32::MAX as usize, f64::from(u32::MAX))]
fn width_as_f64_preserves_values_up_to_u32_max(#[case] width: usize, #[case] expected: f64) {
    assert_eq!(width_as_f64(width).to_bits(), expected.to_bits());
}

#[test]
#[cfg(target_pointer_width = "64")]
fn width_as_f64_clamps_values_larger_than_u32_max() {
    assert_eq!(
        width_as_f64(u32::MAX as usize + 1).to_bits(),
        f64::from(u32::MAX).to_bits(),
    );
}
