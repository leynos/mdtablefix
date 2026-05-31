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
use unicode_width::UnicodeWidthStr;

fn has_md038_code_span(rendered: &str) -> bool {
    let mut remaining = rendered;
    while let Some(open_index) = remaining.find('`') {
        let after_open = &remaining[open_index + 1..];
        let Some(close_index) = after_open.find('`') else {
            break;
        };
        let code = &after_open[..close_index];
        if !code.is_empty() && (code.starts_with(' ') || code.ends_with(' ')) {
            return true;
        }
        remaining = &after_open[close_index + 1..];
    }
    false
}

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

fn checklist_marker_count(lines: &[String]) -> usize {
    lines
        .iter()
        .filter(|line| line.starts_with("- [ ] ") || line.starts_with("- [x] "))
        .count()
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
    fn wrap_text_deferred_checklist_span_does_not_add_checklist_markers(
        checked in any::<bool>(),
        before in "[a-z][a-z ]{0,30}",
        command in "[a-z][a-z0-9_ -]{1,30}",
        suffix in "[a-z][a-z ]{0,30}",
        width in 30usize..=100,
    ) {
        let marker = if checked { "- [x] " } else { "- [ ] " };
        let input = vec![
            format!("{marker}{before} `{command}"),
            format!("  --flag` {suffix}"),
        ];
        let output = wrap_text(&input, width);

        prop_assert_eq!(
            checklist_marker_count(&output),
            1,
            "wrapped checklist item gained markers: {:?}",
            output
        );
    }

    #[test]
    fn wrap_keeps_leading_hyphen_compound_atomic(
        prefix in "\\p{L}{1,12}",
        inner in "\\p{L}{1,12}",
        width in 20usize..120,
    ) {
        let compound = format!("{prefix}-`{inner}`");
        let compound_width = UnicodeWidthStr::width(compound.as_str());

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
        } else {
            prop_assert!(!output.is_empty(), "wrap_text must not panic or return empty output");
        }
    }

    #[test]
    fn wrap_text_opener_at_eol_does_not_create_md038_span(
        body in "[A-Za-z_][A-Za-z0-9_:() .]{1,60}",
        suffix in "[a-z ]{1,40}",
        width in 40usize..=120,
    ) {
        prop_assume!(!body.ends_with(' '));
        let input = vec![
            "4. Opens a code span at line end `".to_string(),
            format!("   {body}`, {suffix}."),
        ];
        let output = wrap_text(&input, width);
        let rendered = output.join("\n");

        prop_assert!(
            !has_md038_code_span(&rendered),
            "output must not contain MD038 code spans:\n{rendered}"
        );
    }

    #[test]
    fn wrap_text_spanning_code_continuations_respect_width_when_source_lines_fit(
        prefix_kind in 0usize..3,
        before in "[a-z]{1,16}",
        part1 in "[a-z]{1,24}",
        part2 in "[a-z]{1,24}",
        width in 24usize..=80,
    ) {
        let (line1_prefix, cont_prefix) = match prefix_kind {
            0 => ("- ".to_owned(), "  ".to_owned()),
            1 => ("1. ".to_owned(), "   ".to_owned()),
            _ => ("> ".to_owned(), "> ".to_owned()),
        };
        let line1 = format!("{line1_prefix}{before} `{part1}");
        let line2 = format!("{cont_prefix}{part2}`");
        let joined = format!("{line1_prefix}{before} `{part1} {part2}`");

        prop_assume!(UnicodeWidthStr::width(line1.as_str()) <= width);
        prop_assume!(UnicodeWidthStr::width(line2.as_str()) <= width);
        prop_assume!(UnicodeWidthStr::width(joined.as_str()) > width);

        let output = wrap_text(&[line1, line2], width);

        for line in &output {
            prop_assert!(
                UnicodeWidthStr::width(line.as_str()) <= width,
                "line exceeds width {width}: {line:?}; output: {output:?}"
            );
        }
    }
}
