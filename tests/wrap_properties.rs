//! Proptest property-based tests for wrapping invariants.
//!
//! These tests exercise `mdtablefix::wrap::wrap_text` with randomly generated
//! inputs and assert that key invariants always hold:
//!
//! - Inline footnote references are never split across lines.
//! - Closing backtick fragments are never orphaned on their own line.
//! - Cross-line inline code spans reflowed under the `PendingPrefix` deferral mechanism produce
//!   atomic output regardless of prefix kind (bullet, ordered list, blockquote) or target width.
//! - Markdown hard-break markers (`  ` trailing spaces) survive flushing of deferred pending-prefix
//!   segments.
//!
//! Related modules:
//! - `tests/wrap/spanning_code_spans.rs` — fixture and unit tests for the same deferral mechanism
//! - `src/wrap/tests/span_state.rs` — unit-level proptest coverage for `has_unclosed_code_span` and
//!   `continuation_begins_with_closing_fence`

use mdtablefix::wrap::wrap_text;
use proptest::prelude::*;

fn footnote_label_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            (b'a'..=b'z').prop_map(char::from),
            (b'A'..=b'Z').prop_map(char::from),
            (b'0'..=b'9').prop_map(char::from),
            Just('-'),
            Just('_')
        ],
        1..16,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #[test]
    fn wrap_text_keeps_generated_footnote_references_atomic(
        label in footnote_label_strategy(),
        prefix_words in 2usize..30,
        suffix_words in 0usize..=20,
        width in 24usize..96,
    ) {
        let marker = format!("[^{label}]");
        let prefix = std::iter::repeat_n("prefix", prefix_words).collect::<Vec<_>>().join(" ");
        let suffix = std::iter::repeat_n("suffix", suffix_words).collect::<Vec<_>>().join(" ");
        let input = if suffix.is_empty() {
            vec![format!("{prefix} xxxxxxxxxxxxxxxxxxxxx.{marker}")]
        } else {
            vec![format!("{prefix} xxxxxxxxxxxxxxxxxxxxx.{marker} {suffix}")]
        };

        let wrapped = wrap_text(&input, width);
        let rendered = wrapped.join("\n");

        prop_assert!(rendered.contains(&marker));
        prop_assert!(!rendered.contains("[\n"));
        prop_assert!(!rendered.contains("\n^"));
    }

    #[test]
    fn wrap_text_never_orphans_closing_backtick(
        content in "[a-z]{1,40}",
        width in 20usize..=100,
    ) {
        let input = vec![format!("- `{content}`")];
        let output = wrap_text(&input, width);
        for line in &output {
            let stripped = line.strip_prefix("  ").unwrap_or(line);
            if stripped.starts_with('`') {
                prop_assert!(
                    stripped.ends_with('`'),
                    "orphaned closing backtick fragment on line: {line:?}"
                );
            }
        }
    }

    #[test]
    fn wrap_text_deferred_span_closing_backtick_not_orphaned_for_generated_prefixes(
        n in 1usize..=3,
        prefix_kind in 0usize..3,
        before in "[a-z]{1,20}",
        inside in "[a-z][a-z ]{0,29}",
        after in "[a-z]{1,20}",
        width in 30usize..=120,
    ) {
        let (line1_prefix, cont_prefix) = match prefix_kind {
            0 => ("- ".to_owned(),  "  ".to_owned()),
            1 => ("1. ".to_owned(), "   ".to_owned()),
            _ => ("> ".to_owned(),  "> ".to_owned()),
        };
        let fence = "`".repeat(n);
        let line1 = format!("{line1_prefix}{before} {fence}{inside}");
        let line2 = format!("{cont_prefix}{inside}{fence} {after}");
        let output = wrap_text(&[line1, line2], width);
        for line in &output {
            let body = line.trim_start_matches(|c: char| {
                c.is_ascii_digit() || matches!(c, ' ' | '-' | '>' | '.')
            });
            if body.starts_with(fence.as_str()) {
                prop_assert!(
                    body.len() > fence.len(),
                    "orphaned closing backtick on line: {line:?}"
                );
            }
        }
    }

    #[test]
    fn wrap_text_deferred_span_preserves_hard_break(
        n in 1usize..=2,
        prefix_kind in 0usize..3,
        before in "[a-z]{1,15}",
        inside in "[a-z][a-z ]{0,19}",
        after in "[a-z]{1,15}",
        width in 50usize..=120,
    ) {
        let fence = "`".repeat(n);
        let (line1_prefix, cont_prefix) = match prefix_kind {
            0 => ("- ".to_owned(), "  ".to_owned()),
            1 => ("1. ".to_owned(), "   ".to_owned()),
            _ => ("> ".to_owned(), "> ".to_owned()),
        };
        let line1 = format!("{line1_prefix}{before} {fence}{inside}");
        // Hard break after the closing fence on the continuation line.
        let line2 = format!("{cont_prefix}{inside}{fence} {after}  ");
        let output = wrap_text(&[line1, line2], width);
        let rendered = output.join("\n");
        prop_assert!(
            output.iter().any(|l| l.ends_with("  ")),
            "hard-break marker lost; rendered:\n{rendered}"
        );
    }

    #[test]
    fn wrap_text_deferred_blockquote_span_stays_atomic(
        n in 1usize..=3,
        before in "[a-z ]{1,25}",
        inside in "[a-z][a-z ]{0,24}",
        after in "[a-z ]{1,25}",
        width in 40usize..=120,
    ) {
        let fence = "`".repeat(n);
        let line1 = format!("> {before} {fence}{inside}");
        let line2 = format!("> {inside}{fence} {after}");
        let output = wrap_text(&[line1, line2], width);
        let bare_closer = format!("> {fence}");
        for line in &output {
            prop_assert!(
                line != &bare_closer,
                "bare closing fence on its own line: {line:?}"
            );
        }
        let rendered = output.join("\n");
        prop_assert!(rendered.contains(&fence), "fence lost; rendered:\n{rendered}");
    }

    #[test]
    fn wrap_keeps_leading_hyphen_compound_atomic(
        prefix in "[a-zA-Z]{1,12}",
        inner in "[a-zA-Z]{1,12}",
        width in 20usize..120,
    ) {
        let compound = format!("{prefix}-`{inner}`");
        let compound_width = prefix.len() + inner.len() + 3;

        let sentence = format!(
            "This sentence has the compound {compound} embedded within it and \
             contains enough trailing prose to force the wrapping algorithm to \
             reflow the text across multiple lines when the target width is \
             sufficiently narrow."
        );
        let input = vec![sentence];
        let output = wrap_text(&input, width);

        if compound_width <= width {
            let rendered = output.join("\n");
            prop_assert!(
                rendered.contains(&compound),
                "compound {compound:?} (width {compound_width}) must appear intact \
                 at target width {width}: {output:?}"
            );
        }
    }
}
