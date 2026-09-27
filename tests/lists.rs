//! Integration tests for list renumbering.

use assert_cmd::Command;
use mdtablefix::renumber_lists;
use rstest::rstest;

#[macro_use]
#[path = "common/mod.rs"]
mod common;

#[test]
fn restart_after_equal_indent_paragraph() {
    // `3. Next` directly follows the paragraph, so it continues it (#573).
    let input = lines_vec!("1. One", "", "Paragraph", "3. Next");
    let expected = lines_vec!("1. One", "", "Paragraph", "3. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn no_restart_without_blank() {
    let input = lines_vec!("1. One", "Paragraph", "3. Next");
    let expected = lines_vec!("1. One", "Paragraph", "2. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn no_restart_for_indented_paragraph() {
    let input = lines_vec!("1. One", "", "  Indented", "3. Next");
    let expected = lines_vec!("1. One", "", "  Indented", "2. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn restart_after_top_heading() {
    let input = lines_vec!("1. One", "", "# Heading", "3. Next");
    let expected = lines_vec!("1. One", "", "# Heading", "1. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn restart_after_nested_paragraph() {
    // `3. Next` directly follows the paragraph, so it continues it (#573).
    let input = lines_vec!("1. One", "    1. Sub", "", "Paragraph", "3. Next");
    let expected = lines_vec!("1. One", "    1. Sub", "", "Paragraph", "3. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn restart_after_nested_equal_indent_paragraph() {
    // `5. Next` directly follows the paragraph, so it continues it (#573).
    let input = lines_vec!("1. One", "    1. Sub", "", "    Paragraph", "    5. Next");
    let expected = lines_vec!("1. One", "    1. Sub", "", "    Paragraph", "    5. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn restart_after_formatting_paragraph() {
    let input = lines_vec!("1. Start", "", "**Bold intro**", "", "4. Next");
    let expected = lines_vec!("1. Start", "", "**Bold intro**", "", "1. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn reset_on_heading_and_thematic_break() {
    let input = lines_vec!("1. a", "2. b", "# Heading", "1. c", "---", "5. d");
    let expected = lines_vec!("1. a", "2. b", "# Heading", "1. c", "---", "1. d");
    assert_eq!(renumber_lists(&input), expected);
}

#[rstest::rstest]
#[case::quoted_break("> ---")]
#[case::quoted_heading("> # Heading")]
fn quoted_structure_does_not_reset_list_numbering(#[case] quoted_line: &str) {
    let input = lines_vec!("1. first", "2. second", quoted_line, "8. third");
    let expected = lines_vec!("1. first", "2. second", quoted_line, "3. third");

    assert_eq!(renumber_lists(&input), expected);
}
/// Tests the CLI `--renumber` option.
///
/// Ensures that list numbering is corrected when the flag is supplied.
#[test]
fn test_cli_renumber_option() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--renumber")
        .write_stdin("1. a\n4. b\n")
        .assert()
        .success()
        .stdout("1. a\n2. b\n");
}

#[test]
fn nested_lists_respect_fence_tracker() {
    let input = lines_vec![
        "1. Outer list",
        "   ```",
        "   1. Code block list",
        "   ```",
        "9. Outer list continued",
        "   4. Nested list",
        "      ```",
        "      - Malformed fence",
        "      ```",
        "   8. Nested list continued",
    ];
    let expected = lines_vec![
        "1. Outer list",
        "   ```",
        "   1. Code block list",
        "   ```",
        "2. Outer list continued",
        // `4.` continues the item's text, so it is not a list (#573); the
        // fence ends that paragraph, so `8.` starts a list, at one.
        "   4. Nested list",
        "      ```",
        "      - Malformed fence",
        "      ```",
        "   1. Nested list continued",
    ];
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn malformed_fences_do_not_break_list_renumbering() {
    let input = lines_vec![
        "1. List before fence",
        "   ```",
        "   1. Inside code block",
        "   ``",
        "   still inside fence",
        "   ```",
        "7. List after fence",
    ];
    let expected = lines_vec![
        "1. List before fence",
        "   ```",
        "   1. Inside code block",
        "   ``",
        "   still inside fence",
        "   ```",
        "2. List after fence",
    ];
    assert_eq!(renumber_lists(&input), expected);
}

#[rstest(
    input,
    expected,
    case::basic(
        lines_vec!["1. first", "2. second", "7. third"],
        lines_vec!["1. first", "2. second", "3. third"]
    ),
    case::with_fence(
        lines_vec!["1. item", "```", "code", "```", "9. next"],
        lines_vec!["1. item", "```", "code", "```", "2. next"]
    ),
    case::nested_lists(
        lines_vec!["1. first", "    1. sub first", "    3. sub second", "2. second"],
        lines_vec!["1. first", "    1. sub first", "    2. sub second", "2. second"]
    ),
    case::tabs_in_indent(
        lines_vec!["1. first", "\t1. sub first", "\t5. sub second", "2. second"],
        lines_vec!["1. first", "\t1. sub first", "\t2. sub second", "2. second"]
    ),
    case::mult_paragraph_items(
        lines_vec!["1. first", "", "    still first paragraph", "", "2. second"],
        lines_vec!["1. first", "", "    still first paragraph", "", "2. second"]
    ),
    case::table_in_list(
        lines_vec!["1. first", "    | A | B |", "    | 1 | 2 |", "5. second"],
        lines_vec!["1. first", "    | A | B |", "    | 1 | 2 |", "2. second"]
    ),
    case::restart_after_paragraph(
        include_lines!("data/renumber_paragraph_restart_input.txt"),
        include_lines!("data/renumber_paragraph_restart_expected.txt")
    ),
    case::restart_after_formatting(
        include_lines!("data/renumber_formatting_paragraph_input.txt"),
        include_lines!("data/renumber_formatting_paragraph_expected.txt")
    ),
    case::restart_after_break(
        include_lines!("data/renumber_break_restart_input.txt"),
        include_lines!("data/renumber_break_restart_expected.txt")
    ),
    case::restart_after_heading(
        include_lines!("data/renumber_heading_restart_input.txt"),
        include_lines!("data/renumber_heading_restart_expected.txt")
    ),
    case::restart_after_break_and_heading(
        include_lines!("data/renumber_break_heading_restart_input.txt"),
        include_lines!("data/renumber_break_heading_restart_expected.txt")
    ),
    case::blank_lines(
        include_lines!("data/renumber_blank_lines_input.txt"),
        include_lines!("data/renumber_blank_lines_expected.txt")
    ),
    case::ordered_list(
        include_lines!("data/renumber_ordered_list_input.txt"),
        include_lines!("data/renumber_ordered_list_expected.txt")
    )
)]
fn test_renumber_cases(input: Vec<String>, expected: Vec<String>) {
    assert_eq!(renumber_lists(&input), expected);
}

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
