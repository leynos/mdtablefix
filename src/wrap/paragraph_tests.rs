//! Unit tests for paragraph wrapping helpers.
//!
//! These tests compile as a child module of `paragraph`, so they can cover
//! private writer behaviour without keeping test-only code in the production
//! module.

use std::borrow::Cow;

use proptest::prelude::*;
use unicode_width::UnicodeWidthStr;

use super::{
    ContinuationMode,
    ParagraphState,
    ParagraphWriter,
    PendingPrefix,
    PrefixLine,
    pending_prefix_for_next_segment,
};

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
            outer_prefix: None,
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
            outer_prefix: None,
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

#[test]
fn pending_prefix_first_call_returns_original_prefix_and_marks_used() {
    let mut pending = pending_prefix("- [ ] ", false);

    let prefix = pending_prefix_for_next_segment(&mut pending);

    assert_eq!(prefix, "- [ ] ");
    assert!(pending.used_prefix);
}

#[test]
fn pending_prefix_subsequent_call_returns_continuation_indent() {
    let mut pending = pending_prefix("- [ ] ", false);

    let _ = pending_prefix_for_next_segment(&mut pending);
    let prefix = pending_prefix_for_next_segment(&mut pending);

    assert_eq!(prefix, "      ");
    assert!(pending.used_prefix);
}

#[test]
fn pending_prefix_repeat_prefix_returns_original_prefix_every_time() {
    let mut pending = pending_prefix("> ", true);

    let first = pending_prefix_for_next_segment(&mut pending);
    let second = pending_prefix_for_next_segment(&mut pending);

    assert_eq!(first, "> ");
    assert_eq!(second, "> ");
    assert!(pending.used_prefix);
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

fn pending_prefix(prefix: &str, repeat_prefix: bool) -> PendingPrefix {
    PendingPrefix {
        prefix: prefix.to_string(),
        rest: "text".to_string(),
        original_lines: vec![format!("{prefix}text")],
        synthetic_join_spaces: Vec::new(),
        rest_width: 74,
        repeat_prefix,
        outer_prefix: None,
        hard_break: false,
        open_fence_len: Some(1),
        continuation_mode: ContinuationMode::Normalize,
        used_prefix: false,
    }
}
