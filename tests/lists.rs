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
    // A fence at the list's marker column ends the list (issue #563).
    case::with_fence(
        lines_vec!["1. item", "```", "code", "```", "9. next"],
        lines_vec!["1. item", "```", "code", "```", "1. next"]
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
#[case::nested_break(include_lines!("data/issue_450_nested_break_input.txt"))]
fn renumber_issue_450_block_inside_an_item_keeps_the_list_counting(#[case] input: Vec<String>) {
    let once = renumber_lists(&input);
    assert_eq!(once, input);
    assert_eq!(renumber_lists(&once), once, "a second pass changes nothing");
}

/// Regression cases for issue #450, the neighbouring shapes #106 touched: a
/// heading or break ends exactly the lists whose current item it is not
/// indented into.
///
/// #106 reset on every ATX heading and thematic break indented zero to three
/// spaces. Under `1. `, whose content starts at column 3, a heading at three
/// spaces is inside the item and the list continues; at one or two spaces it
/// is not, so the list ends as it does at column 0, which #106 intended. A
/// block inside the outer item but left of a nested item's content ends the
/// nested list only.
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
#[case::heading_three_spaces_inside_item(
    lines_vec!["1. a", "", "   ### h", "", "5. b"],
    lines_vec!["1. a", "", "   ### h", "", "2. b"]
)]
#[case::heading_two_spaces_ends_list(
    lines_vec!["1. a", "", "  ### h", "", "5. b"],
    lines_vec!["1. a", "", "  ### h", "", "1. b"]
)]
#[case::heading_one_space_ends_list(
    lines_vec!["1. a", "", " ### h", "", "5. b"],
    lines_vec!["1. a", "", " ### h", "", "1. b"]
)]
#[case::break_two_spaces_ends_list(
    lines_vec!["1. a", "", "  ***", "", "5. b"],
    lines_vec!["1. a", "", "  ***", "", "1. b"]
)]
// Inside the outer item but left of the nested item's content: the heading
// reads as a heading relative to the outer item, so only the nested list ends.
#[case::heading_at_column_four_between_items(
    lines_vec![
        "1. Outer", "   1. Inner", "", "    #### Heading", "", "   5. Inner again", "7. Outer again",
    ],
    lines_vec![
        "1. Outer", "   1. Inner", "", "    #### Heading", "", "   1. Inner again", "2. Outer again",
    ]
)]
// A tab in the separator advances to the next tab stop, so `1. \t` puts the
// content at column 4 and a three-space heading is outside the item.
#[case::heading_left_of_a_tab_separated_item(
    lines_vec!["1. \titem", "", "   ### h", "", "5. b"],
    lines_vec!["1. \titem", "", "   ### h", "", "1. b"]
)]
// The content column is measured on the emitted marker, so `10.` becoming
// `2.` puts the heading inside the item on the first pass as on the second.
#[case::heading_after_a_narrowed_marker(
    lines_vec!["9. a", "10. b", "", "   ### h", "", "4. c"],
    lines_vec!["1. a", "2. b", "", "   ### h", "", "3. c"]
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

/// Regression cases for issue #563: a block that ends an ordered list resets
/// numbering for the next list.
///
/// Each fixture is a list, then a block at the list's marker column, then a
/// second list that the source starts at one. `CommonMark` ends the first list
/// at that block, so renumbering must leave the second list at one; carrying
/// the count across it changes the rendered `start` of the second list.
#[rstest]
#[case::fence(include_lines!("data/issue_563_fence_input.txt"))]
#[case::bullet_list(include_lines!("data/issue_563_bullet_list_input.txt"))]
#[case::table(include_lines!("data/issue_563_table_input.txt"))]
#[case::block_quote(include_lines!("data/issue_563_block_quote_input.txt"))]
#[case::html_comment(include_lines!("data/issue_563_html_comment_input.txt"))]
#[case::link_paragraph(include_lines!("data/issue_563_link_paragraph_input.txt"))]
fn renumber_issue_563_block_at_marker_column_ends_the_list(#[case] input: Vec<String>) {
    let once = renumber_lists(&input);
    assert_eq!(once, input);
    assert_eq!(renumber_lists(&once), once, "a second pass changes nothing");
}

/// Regression cases for issue #563, the neighbouring shapes: a block
/// indented into an item, or a bullet item that interrupts without a blank
/// line, is decided by its column alone.
///
/// A block right of the list's marker column belongs to the item and the
/// list keeps counting; one at a nested list's marker column ends only that
/// nested list; a bullet item at the marker column ends the list even with
/// no blank line before it.
#[rstest]
#[case::fence_inside_item(
    lines_vec!["1. a", "   ```", "   code", "   ```", "5. b"],
    lines_vec!["1. a", "   ```", "   code", "   ```", "2. b"]
)]
#[case::bullet_inside_item(
    lines_vec!["1. a", "", "   - sub", "", "5. b"],
    lines_vec!["1. a", "", "   - sub", "", "2. b"]
)]
#[case::fence_at_nested_marker_column(
    lines_vec![
        "1. Outer", "   1. Inner", "   2. Inner", "", "   ```", "   code", "   ```", "",
        "   4. Inner again", "5. Outer again",
    ],
    lines_vec![
        "1. Outer", "   1. Inner", "   2. Inner", "", "   ```", "   code", "   ```", "",
        "   1. Inner again", "2. Outer again",
    ]
)]
// The bullet ends the list without a blank line above it, so the list after
// the blank line restarts. (A `3. c` directly below the bullet is not a list
// item at all: it lazily continues the bullet's paragraph, see #573.)
#[case::bullet_interrupts_without_blank(
    lines_vec!["1. a", "2. b", "- bullet", "", "3. c"],
    lines_vec!["1. a", "2. b", "- bullet", "", "1. c"]
)]
#[case::lazy_paragraph_continues(
    lines_vec!["1. a", "[lazy](u) continuation", "3. b"],
    lines_vec!["1. a", "[lazy](u) continuation", "2. b"]
)]
fn renumber_issue_563_neighbouring_shapes(
    #[case] input: Vec<String>,
    #[case] expected: Vec<String>,
) {
    let once = renumber_lists(&input);
    assert_eq!(once, expected);
    assert_eq!(renumber_lists(&once), once, "a second pass changes nothing");
}

/// Regression case for issue #563 through the CLI: `--renumber` leaves the
/// list after an ending fence at one.
#[test]
fn renumber_issue_563_cli_keeps_the_restart_after_a_fence() {
    let input = include_str!("data/issue_563_fence_input.txt");
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--renumber")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(input);
}
