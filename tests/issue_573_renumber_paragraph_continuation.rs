//! Regression tests for issue #573.
//!
//! Invariant: `--renumber` never changes whether a line is a list item. Only
//! a list starting at 1 can interrupt a paragraph, so a numbered line that
//! follows paragraph text continues that paragraph and keeps its marker.

use assert_cmd::Command;
use mdtablefix::renumber_lists;
use rstest::rstest;

#[macro_use]
#[path = "common/mod.rs"]
mod common;

/// Regression cases for issue #573: renumbering never changes whether a line
/// is a list item.
///
/// Only a list starting at 1 can interrupt a paragraph, so a line such as
/// `12. Evidence …` that follows paragraph text continues the paragraph.
/// Rewriting its marker to `1.` would turn that text into a list item, so it
/// is left exactly as written.
#[rstest]
#[case::wrapped_sentence(include_lines!("data/issue_573_paragraph_continuation_input.txt"))]
#[case::inside_an_item(lines_vec!["1. First", "   continues here and ends with section", "   8. Work item text"])]
#[case::after_an_item_line(lines_vec!["1. a", "   2. b"])]
// Paragraph text indented four or more columns inside an item is still text.
#[case::deep_in_an_item(lines_vec!["  1. Snapshots store", "     wrapped text", "     12. Evidence"])]
fn renumber_issue_573_paragraph_continuation_is_not_an_item(#[case] input: Vec<String>) {
    let once = renumber_lists(&input);
    assert_eq!(once, input);
    assert_eq!(renumber_lists(&once), once, "a second pass changes nothing");
}

/// Regression cases for issue #573, the neighbouring shapes: a numbered line
/// that does start or continue a list is still renumbered.
///
/// A sibling of an active list continues it whatever the paragraph above,
/// `1.` can interrupt a paragraph, and after a blank line or heading any
/// number starts a list.
#[rstest]
#[case::sibling_after_item_text(lines_vec!["1. a", "5. b"], lines_vec!["1. a", "2. b"])]
#[case::nested_sibling(
    lines_vec!["1. a", "   1. b", "   5. c"],
    lines_vec!["1. a", "   1. b", "   2. c"]
)]
#[case::one_interrupts(lines_vec!["text", "1. b", "4. c"], lines_vec!["text", "1. b", "2. c"])]
#[case::after_a_blank_line(lines_vec!["text", "", "4. b"], lines_vec!["text", "", "1. b"])]
#[case::after_a_heading(lines_vec!["# h", "4. b"], lines_vec!["# h", "1. b"])]
fn renumber_issue_573_neighbouring_shapes(
    #[case] input: Vec<String>,
    #[case] expected: Vec<String>,
) {
    let once = renumber_lists(&input);
    assert_eq!(once, expected);
    assert_eq!(renumber_lists(&once), once, "a second pass changes nothing");
}

/// Regression case for issue #573 through the CLI: `--renumber` leaves the
/// wrapped sentence's `12.` alone.
#[test]
fn renumber_issue_573_cli_leaves_paragraph_text_alone() {
    let input = include_str!("data/issue_573_paragraph_continuation_input.txt");
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--renumber")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(input);
}
