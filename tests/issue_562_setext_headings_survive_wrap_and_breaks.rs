//! Regression tests for issue #562.
//!
//! Invariant: Setext headings survive `--wrap` and `--breaks`.
//!
//! A Setext heading is paragraph text followed by an underline of `=` or `-`.
//! `--wrap` used to join a `===` underline onto its text as one paragraph, and
//! `--breaks` rewrote a `---` underline as a seventy-underscore thematic break;
//! either way the heading rendered as a paragraph. Each reproduction from the
//! issue runs through the real binary under each flag and under both together,
//! as `make fmt` runs them. The heading's text line and underline must come
//! out as they went in, and a second pass must change nothing.

use assert_cmd::Command;
use proptest::prelude::*;
use rstest::rstest;

/// Formats `input` through the binary with `flags` and returns stdout.
fn format(input: &str, flags: &[&str]) -> String {
    let output = Command::cargo_bin("mdtablefix")
        .expect("the mdtablefix binary builds")
        .args(flags)
        .write_stdin(input)
        .output()
        .expect("mdtablefix runs");
    assert!(output.status.success(), "mdtablefix failed: {output:?}");
    String::from_utf8(output.stdout).expect("mdtablefix writes UTF-8")
}

/// The reproduction from the issue: one heading of each level.
const GAMBIT: &str = "Arc Gambit\n==========\n\nWhat is it?\n-----------\n\nAn SRPG.\n";

/// Scenario: the issue's two headings under each flag set.
///
/// Invariant: both text lines and both underlines survive unchanged, and the
/// output is a fixed point.
#[rstest]
#[case::wrap(&["--wrap"])]
#[case::breaks(&["--breaks"])]
#[case::wrap_and_breaks(&["--wrap", "--breaks"])]
#[case::make_fmt_flags(&["--wrap", "--renumber", "--breaks", "--ellipsis", "--fences"])]
fn setext_headings_survive(#[case] flags: &[&str]) {
    let first = format(GAMBIT, flags);
    let lines: Vec<&str> = first.lines().collect();
    for heading in ["Arc Gambit", "==========", "What is it?", "-----------"] {
        assert!(lines.contains(&heading), "{heading:?} was lost: {first:?}");
    }
    assert_eq!(
        format(&first, flags),
        first,
        "a second pass changed the output"
    );
}

/// Scenario: a real thematic break beside a Setext heading.
///
/// Invariant: `--breaks` still normalizes a break that follows a blank line,
/// so the fix is narrow: only an underline directly below paragraph text is
/// left alone.
#[test]
fn a_break_after_a_blank_line_is_still_normalized() {
    let output = format("Heading\n-------\n\nText.\n\n---\n\nMore.\n", &["--breaks"]);
    let underscores = "_".repeat(mdtablefix::THEMATIC_BREAK_LEN);
    assert!(
        output.lines().any(|line| line == "-------"),
        "underline lost: {output:?}"
    );
    assert!(
        output.lines().any(|line| line == underscores),
        "break not normalized: {output:?}"
    );
}

/// Scenario: heading text longer than the wrap width, and an underline with
/// paragraph text directly below it.
///
/// Invariant: `--wrap` leaves a Setext heading's text on one line, as it
/// leaves ATX heading text, and never joins the underline to the paragraph
/// that follows it.
#[rstest]
#[case::long_heading(
    concat!(
        "A heading whose text runs well past the eighty column wrap width of the ",
        "formatter\n==========\n",
    ),
    &["==========", concat!(
        "A heading whose text runs well past the eighty column wrap width of the ",
        "formatter",
    )],
)]
#[case::text_after_underline(
    "Title\n=====\nBody text follows the heading directly.\n",
    &["Title", "=====", "Body text follows the heading directly."],
)]
fn a_setext_heading_passes_through_whole(#[case] input: &str, #[case] kept: &[&str]) {
    let output = format(input, &["--wrap"]);
    for line in kept {
        assert!(
            output.lines().any(|l| l == *line),
            "{line:?} was changed: {output:?}"
        );
    }
    assert_eq!(
        format(&output, &["--wrap"]),
        output,
        "a second pass changed the output"
    );
}

proptest! {
    /// Any single-line Setext heading survives the `make fmt` flags and is a
    /// fixed point.
    #[test]
    fn any_setext_heading_survives(
        title in "[A-Z][a-z]{1,8}( [a-z]{1,8}){0,6}",
        marker in prop_oneof![Just('='), Just('-')],
        length in 3_usize..40,
    ) {
        let underline = marker.to_string().repeat(length);
        let input = format!("Intro text.\n\n{title}\n{underline}\n\nBody text.\n");
        let flags = ["--wrap", "--renumber", "--breaks", "--ellipsis", "--fences"];
        let first = format(&input, &flags);
        prop_assert!(first.lines().any(|line| line == title), "title lost: {:?}", first);
        prop_assert!(first.lines().any(|line| line == underline), "underline lost: {:?}", first);
        prop_assert_eq!(format(&first, &flags), first);
    }
}
