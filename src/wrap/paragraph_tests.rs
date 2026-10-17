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
use crate::{process::WRAP_COLS, wrap::wrap_text};

/// The list continuation indent a deferred bullet item reuses.
const LIST_CONTINUATION_INDENT: &str = "  ";

/// The two documents the widened issue #493 sweep shrank its drift to.
///
/// Each is a bullet item whose overlong first line is deferred — its tail
/// reflows with the lines below it — and whose joined paragraph ends with a
/// hard break: two trailing spaces in the first document, a trailing
/// backslash in the second. The first also carries a code span wider than the
/// wrap width, which is what the sweep's paragraph element contributed.
const SHRUNK_DRIFT_DOCUMENTS: &[&[&str]] = &[
    &[
        "- aaaa aaaaaa aaaaaaa aaaaa aa aa yuslmwa rco (mozsfzb elo mvezlum lcedwv vzzfqs)",
        "  sw",
        "rr yzrbvin",
        "hwcjtt qqslqrti yfo lkcahvz upt  ",
        "hlepyw xvrsqmjs qobvb ap \
         `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
         xxxxxxxxxxxxxxxxxxxxxxxx` vzwi wxjqqnb kstoebq ybrhxptt jor xyo azqdhnd zred mxr",
    ],
    &[
        "- aaaaaaa aaaaaaa aaaa aaaaa abhi icwzwn tlwtc gvxrfi osdt uoj wn zvjzc tbwklp fx",
        "gityvz yhedqomw wgxxridm hmfxp xhwl dcvxgoi gmlbowiv qdjfqw aatedp",
        "mvcqmbh orgva iraf ykduq jbxi xyk",
        "gpllvp xqpjmpy spjkj lhacuycz jyf algs\\",
        "xyjoxbut\\",
        "cqt hmizwk ad je jeov umu oseomxt oymopk",
    ],
];

/// Asserts a deferred bullet item indents the lazy lines below a hard break.
///
/// The first line is overlong, so its tail is deferred and reflowed with the
/// lines below it; the flush that ends on the hard break remembers the item's
/// continuation indent. The line below the break is a lazy continuation of the
/// same item, so it must be emitted with that indent. It was emitted
/// flush-left instead and re-indented on the next pass, so the source had no
/// fixed point.
#[test]
fn deferred_list_item_indents_the_lazy_line_below_a_hard_break() {
    let input: Vec<String> = [
        "- alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha beta",
        "delta epsilon  ",
        "zeta eta",
    ]
    .iter()
    .map(|line| (*line).to_owned())
    .collect();

    let once = wrap_text(&input, WRAP_COLS);

    assert_eq!(
        once,
        vec![
            "- alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha",
            "  beta delta epsilon  ",
            "  zeta eta",
        ],
    );
    assert_eq!(
        wrap_text(&once, WRAP_COLS),
        once,
        "wrap is not a fixed point"
    );
}

/// Asserts the two documents the sweep shrank its failure to are fixed points
/// whose lazy lines keep the item's continuation indent.
#[test]
fn shrunk_drift_documents_are_fixed_points_with_indented_lazy_lines() {
    for (index, document) in SHRUNK_DRIFT_DOCUMENTS.iter().enumerate() {
        let input: Vec<String> = document.iter().map(|line| (*line).to_owned()).collect();
        let once = wrap_text(&input, WRAP_COLS);

        assert_eq!(
            wrap_text(&once, WRAP_COLS),
            once,
            "document {index} is not a fixed point:\n{}",
            once.join("\n"),
        );
        for line in once.iter().skip(1) {
            assert!(
                line.starts_with(LIST_CONTINUATION_INDENT),
                "document {index} lost the list indent on {line:?}",
            );
        }
    }
}

/// The document the 4000-case sweep shrank the backslash-tail overflow to.
///
/// A list item whose overlong first line is deferred, and whose joined
/// paragraph ends with a backslash hard break. The two documents below are the
/// same shape at two sizes: the first fits the reflow in three continuation
/// lines, the second is the sweep's own shrink.
const SHRUNK_BACKSLASH_TAIL_DOCUMENTS: &[&[&str]] = &[
    &[
        "1. aaaaa aaaaa aaaaa aaaaa aaaaa aaaaa aaaaa aaaaa aaaaa aaaaa aaaaa aaaaa aaaaa aaaaa",
        "bbbbb bbbbb bbbbb bbbbb bbbbb bbbbb bbbbb bbbbb bbbbb bbbbb bbbbb\\",
    ],
    &[
        "1. aaaaa aaaaaaaa aaaaaaa aaaaa aa aaaaaaa aaaaaaaa aaaa aaa aaaa aaaaa aaaaaaaa aa \
         aaaaaaa aaaa",
        "aa aaaaaa aaa aaaaaa aaaa aaaaaaa aa aaaaaaa aaaaaaa",
        "aaaaa aaaaa aaaaaaaa aa",
        "aaa aaaaaaa aaaa aaaaaaaa aaaa aaaaaaaa aaaaa\\",
    ],
];

/// Asserts a deferred tail measures a backslash hard break as content.
///
/// The tail of a deferred prefix is rewrapped on its own and the Markdown
/// hard-break marker is re-appended afterwards. A backslash marker ends up
/// glued to the last word of the source line, so it is content: the next pass
/// reads it back as part of that word and measures it. Appending it after the
/// wrap spent the whole width first, so the emitted line grew one column past
/// the width and the next pass, which did measure the backslash, wrapped one
/// word earlier. The marker is now left in the text handed to the wrapper for
/// the backslash case, and still stripped and re-appended for a whitespace
/// marker, which the next pass trims before measuring.
#[test]
fn deferred_tail_measures_a_backslash_hard_break_as_content() {
    for (index, document) in SHRUNK_BACKSLASH_TAIL_DOCUMENTS.iter().enumerate() {
        let input: Vec<String> = document.iter().map(|line| (*line).to_owned()).collect();
        let once = wrap_text(&input, WRAP_COLS);

        assert_eq!(
            wrap_text(&once, WRAP_COLS),
            once,
            "document {index} is not a fixed point:\n{}",
            once.join("\n"),
        );
        for line in &once {
            let width = UnicodeWidthStr::width(line.as_str());
            assert!(
                width <= WRAP_COLS,
                "document {index} emitted a {width}-column line: {line:?}",
            );
        }
        assert!(
            once.last().is_some_and(|line| line.ends_with('\\')),
            "document {index} lost the hard break: {once:?}",
        );
    }
}

#[test]
fn wrap_with_prefix_emits_single_line_when_text_fits() {
    let mut out = Vec::new();
    let mut writer = ParagraphWriter::new(&mut out, 80);
    writer.wrap_with_prefix("> ", "> ", "hello world");
    assert_eq!(out, vec!["> hello world".to_owned()]);
}

#[test]
fn wrap_with_prefix_uses_continuation_prefix_on_wrapped_lines() {
    let mut out = Vec::new();
    let mut writer = ParagraphWriter::new(&mut out, 14);
    writer.wrap_with_prefix("> ", "  ", "alpha beta gamma");
    assert_eq!(out, vec!["> alpha beta".to_owned(), "  gamma".to_owned()]);
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
    assert_eq!(out, vec!["「 ab".to_owned(), "  cd".to_owned()]);
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
        prefix: prefix.to_owned(),
        rest: "text".to_owned(),
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
