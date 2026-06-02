//! Unit tests for paragraph wrapping helpers.
//!
//! These tests compile as a child module of `paragraph`, so they can cover
//! private writer behaviour without keeping test-only code in the production
//! module.

use std::borrow::Cow;

use proptest::prelude::*;
use unicode_width::UnicodeWidthStr;

use super::{ParagraphState, ParagraphWriter, PrefixLine};

#[test]
fn wrap_with_prefix_emits_single_line_when_text_fits() {
    let mut out = Vec::new();
    let mut writer = ParagraphWriter::new(&mut out, 80);
    writer.wrap_with_prefix("> ", "> ", "hello world");
    assert_eq!(out, vec!["> hello world".to_string()]);
}

#[test]
fn wrap_with_prefix_uses_continuation_prefix_on_wrapped_lines() {
    let mut out = Vec::new();
    let mut writer = ParagraphWriter::new(&mut out, 14);
    writer.wrap_with_prefix("> ", "  ", "alpha beta gamma");
    assert_eq!(out, vec!["> alpha beta".to_string(), "  gamma".to_string()]);
}

#[test]
fn handle_prefix_line_can_repeat_or_change_the_continuation_prefix() {
    let mut out = Vec::new();
    let mut writer = ParagraphWriter::new(&mut out, 14);
    let mut state = ParagraphState::default();
    writer.handle_prefix_line(
        &mut state,
        &PrefixLine {
            prefix: Cow::Borrowed("- [ ] "),
            rest: "alpha beta",
            repeat_prefix: false,
        },
    );
    assert_eq!(
        out,
        vec!["- [ ] alpha".to_string(), "      beta".to_string()]
    );

    let mut quoted_out = Vec::new();
    let mut quoted_writer = ParagraphWriter::new(&mut quoted_out, 10);
    let mut quoted_state = ParagraphState::default();
    quoted_writer.handle_prefix_line(
        &mut quoted_state,
        &PrefixLine {
            prefix: Cow::Borrowed("> "),
            rest: "alpha beta gamma",
            repeat_prefix: true,
        },
    );
    assert_eq!(
        quoted_out,
        vec![
            "> alpha".to_string(),
            "> beta".to_string(),
            "> gamma".to_string(),
        ]
    );
}

#[test]
fn wrap_with_prefix_accounts_for_unicode_wide_prefixes() {
    let mut out = Vec::new();
    let mut writer = ParagraphWriter::new(&mut out, 7);
    writer.wrap_with_prefix("「 ", "  ", "ab cd");
    assert_eq!(out, vec!["「 ab".to_string(), "  cd".to_string()]);
}

proptest! {
    #[test]
    fn paragraph_writer_preserves_prefixes_and_width(
        words in proptest::collection::vec("[a-z]{1,6}", 1..=8),
        width in 20usize..=60,
        indent in 0usize..=4,
    ) {
        let prefix = format!("{}- ", " ".repeat(indent));
        let continuation = " ".repeat(UnicodeWidthStr::width(prefix.as_str()));
        let text = words.join(" ");
        let mut out = Vec::new();
        let mut writer = ParagraphWriter::new(&mut out, width);

        writer.wrap_with_prefix(&prefix, &continuation, &text);

        prop_assert!(!out.is_empty());
        prop_assert!(out[0].starts_with(&prefix));
        for line in out.iter().skip(1) {
            prop_assert!(line.starts_with(&continuation));
        }
        for line in &out {
            prop_assert!(
                UnicodeWidthStr::width(line.as_str()) <= width,
                "wrapped line exceeded width {width}: {line:?}",
            );
        }
    }
}
