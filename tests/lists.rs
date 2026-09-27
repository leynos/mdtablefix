//! Integration tests for list renumbering.

use assert_cmd::Command;
use mdtablefix::renumber_lists;
use rstest::rstest;

#[macro_use]
#[path = "common/mod.rs"]
mod common;

#[test]
fn restart_after_equal_indent_paragraph() {
    let input = lines_vec!("1. One", "", "Paragraph", "3. Next");
    let expected = lines_vec!("1. One", "", "Paragraph", "1. Next");
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
    let input = lines_vec!("1. One", "    1. Sub", "", "Paragraph", "3. Next");
    let expected = lines_vec!("1. One", "    1. Sub", "", "Paragraph", "1. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn restart_after_nested_equal_indent_paragraph() {
    let input = lines_vec!("1. One", "    1. Sub", "", "    Paragraph", "    5. Next");
    let expected = lines_vec!("1. One", "    1. Sub", "", "    Paragraph", "    1. Next");
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
        "   1. Nested list",
        "      ```",
        "      - Malformed fence",
        "      ```",
        "   2. Nested list continued",
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

/// Regression cases for issue #450: a heading indented inside a list item
/// does not end the ordered list.
///
/// Issue #450 was introduced by #106 (1b41d7f), which made every ATX heading
/// and thematic break up to three spaces deep reset all list state. A heading
/// or break indented to an item's content column is a child block of that
/// item, so the items after it keep their numbers.
#[rstest]
#[case::nested_heading(include_lines!("data/issue_450_nested_heading_input.txt"))]
// Inline rather than under `tests/data`: the drift harness runs every fixture
// there under `--breaks`, which rewrites this break at column 0 (reported
// separately), so it is not a fixed point of the full flag set.
#[case::nested_break(lines_vec![
    "1. First item", "", "   Body paragraph.", "", "2. Second item", "", "   ***", "",
    "   More body after a break inside the item.", "", "3. Third item",
])]
fn renumber_issue_450_block_inside_an_item_keeps_the_list_counting(#[case] input: Vec<String>) {
    let once = renumber_lists(&input);
    assert_eq!(once, input);
    assert_eq!(renumber_lists(&once), once, "a second pass changes nothing");
}

/// Regression cases for issue #450, the neighbouring shapes #106 touched: a
/// heading or break ends exactly the lists at or right of its column.
///
/// At a nested list's marker column it ends that nested list only, and the
/// outer list keeps counting; at column 0 it ends every list.
#[rstest]
#[case::heading_at_nested_marker_column(
    lines_vec![
        "1. Outer", "   1. Inner", "   2. Inner", "", "   ## Heading in the outer item", "",
        "   5. Inner again", "7. Outer again",
    ],
    lines_vec![
        "1. Outer", "   1. Inner", "   2. Inner", "", "   ## Heading in the outer item", "",
        "   1. Inner again", "2. Outer again",
    ]
)]
#[case::break_at_nested_marker_column(
    lines_vec!["1. Outer", "   1. Inner", "", "   ---", "", "   4. Inner again", "6. Outer again"],
    lines_vec!["1. Outer", "   1. Inner", "", "   ---", "", "   1. Inner again", "2. Outer again"]
)]
#[case::heading_at_margin(
    lines_vec!["1. a", "4. b", "", "# Title", "", "6. c", "9. d"],
    lines_vec!["1. a", "2. b", "", "# Title", "", "1. c", "2. d"]
)]
#[case::break_at_margin(
    lines_vec!["1. a", "   1. sub", "", "---", "", "6. c"],
    lines_vec!["1. a", "   1. sub", "", "---", "", "1. c"]
)]
fn renumber_issue_450_neighbouring_shapes(
    #[case] input: Vec<String>,
    #[case] expected: Vec<String>,
) {
    let once = renumber_lists(&input);
    assert_eq!(once, expected);
    assert_eq!(renumber_lists(&once), once, "a second pass changes nothing");
}

/// Regression case for issue #450 through the CLI: `--renumber` keeps the
/// list counting past a heading nested in an item.
#[test]
fn renumber_issue_450_cli_keeps_counting_past_a_nested_heading() {
    let input = include_str!("data/issue_450_nested_heading_input.txt");
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--renumber")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(input);
}
