//! Unit tests for inline fragment classification helpers.

use rstest::rstest;
use unicode_width::UnicodeWidthStr;

use super::fragment::{FragmentKind, InlineFragment};

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
