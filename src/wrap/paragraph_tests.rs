//! Unit tests for paragraph wrapping helpers.
//!
//! These tests compile as a child module of `paragraph`, so they can cover
//! private writer behaviour without keeping test-only code in the production
//! module.

use std::borrow::Cow;

use proptest::prelude::*;
use rstest::rstest;
use unicode_width::UnicodeWidthStr;

use super::{
    ContinuationMode,
    ParagraphState,
    ParagraphWriter,
    PendingPrefix,
    PrefixLine,
    TailReflow,
    continuation_folds_tail,
    pending_prefix_for_next_segment,
    wraps_to_tail,
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

#[rstest]
#[case::plain_list(14, "- [ ] ", "alpha beta", false, None, "- [ ] alpha\n      beta")]
#[case::repeated_quote(10, "> ", "alpha beta gamma", true, None, "> alpha\n> beta\n> gamma")]
#[case::quoted_list(
    10,
    "> - ",
    "alpha beta gamma",
    false,
    Some("> "),
    "> - alpha\n>   beta\n>   gamma"
)]
fn handle_prefix_line_can_repeat_or_change_the_continuation_prefix(
    #[case] width: usize,
    #[case] prefix: &str,
    #[case] rest: &str,
    #[case] repeat_prefix: bool,
    #[case] outer_prefix: Option<&str>,
    #[case] expected: &str,
) {
    let mut out = Vec::new();
    let mut writer = ParagraphWriter::new(&mut out, width);
    let mut state = ParagraphState::default();
    writer.handle_prefix_line(
        &mut state,
        &PrefixLine {
            prefix: Cow::Borrowed(prefix),
            rest,
            repeat_prefix,
            outer_prefix: outer_prefix.map(Cow::Borrowed),
        },
    );
    // The list cases spill onto a continuation line, so their emission is
    // deferred until the paragraph flushes; the continuation prefix under test
    // is chosen during the flush. The repeated-quote case emits immediately and
    // is unaffected by the flush.
    writer.flush_paragraph(&mut state);
    assert_eq!(out.join("\n"), expected);
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

#[rstest]
#[case::empty("", true)]
#[case::one_space(" ", true)]
#[case::three_spaces("   ", true)]
#[case::list_indent("  ", true)]
#[case::code_threshold("    ", false)]
#[case::tab("\t", false)]
#[case::blockquote("> ", false)]
#[case::nested_blockquote(">   ", false)]
#[case::footnote_indent("      ", false)]
fn continuation_prefix_folds_its_tail_only_when_it_is_narrow_space(
    #[case] prefix: &str,
    #[case] expected: bool,
) {
    assert_eq!(continuation_folds_tail(prefix), expected);
}

#[rstest]
#[case::fits("alpha beta", 80, false)]
#[case::spills("alpha beta gamma delta", 10, true)]
#[case::empty("", 10, false)]
fn text_spills_onto_a_tail_only_when_it_wraps(
    #[case] text: &str,
    #[case] available: usize,
    #[case] expected: bool,
) {
    assert_eq!(wraps_to_tail(text, available), expected);
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
        tail_reflow: TailReflow::Allowed,
    }
}
