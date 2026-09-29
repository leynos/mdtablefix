//! Integration tests for formatting thematic breaks.
//!
//! Verifies `format_breaks` function and `--breaks` CLI option.

use assert_cmd::Command;
use mdtablefix::{THEMATIC_BREAK_LEN, format_breaks};
use rstest::rstest;

#[macro_use]
#[path = "common/mod.rs"]
mod common;

macro_rules! assert_borrowed_break {
    ($line:expr $(,)?) => {
        assert_borrowed_value!($line, &"_".repeat(THEMATIC_BREAK_LEN));
    };
}

macro_rules! assert_borrowed_value {
    ($line:expr, $expected:expr $(,)?) => {
        match &$line {
            std::borrow::Cow::Borrowed(value) => assert_eq!(*value, $expected),
            std::borrow::Cow::Owned(value) => {
                panic!("expected borrowed value, got owned {value:?}")
            }
        }
    };
}

#[test]
fn test_format_breaks_basic() {
    let input = lines_vec!["foo", "***", "bar"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], "foo");
    assert_borrowed_break!(output[1]);
    assert_borrowed_value!(output[2], "bar");
}

#[test]
fn test_format_breaks_preserves_blockquote_prefix() {
    let input = lines_vec!["> ---", "> > ***"];
    let output = format_breaks(&input);

    assert_eq!(output[0], format!("> {}", "_".repeat(THEMATIC_BREAK_LEN)));
    assert_eq!(output[1], format!("> > {}", "_".repeat(THEMATIC_BREAK_LEN)));
}

/// A Setext underline remains borrowed source text without the headings pass.
#[rstest]
#[case("Title", "---")]
#[case("> Title", "> ---")]
#[case("  Title", " ---")]
fn leaves_setext_underlines_unchanged(#[case] title: &str, #[case] underline: &str) {
    let input = lines_vec![title, underline];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], title);
    assert_borrowed_value!(output[1], underline);
    assert!(std::ptr::eq(output[1].as_ref(), input[1].as_str()));
}

/// A blank line breaks the Setext pair, leaving a standalone thematic break.
#[test]
fn normalizes_break_after_blank_line() {
    let input = lines_vec!["Title", "", "---"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[1], "");
    assert_borrowed_break!(output[2]);
}

/// An outdented break ends a list continuation rather than underlining it.
#[test]
fn normalizes_break_after_outdented_list_continuation() {
    let input = lines_vec!["- item", "  continuation", "---"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], "- item");
    assert_borrowed_value!(output[1], "  continuation");
    assert_borrowed_break!(output[2]);
}

/// Multiple marker separators move the list content beyond a two-space break.
#[test]
fn normalizes_break_below_wide_list_separator() {
    let input = lines_vec!["-   Bar", "  ---"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], "-   Bar");
    assert_borrowed_break!(output[1]);
}

/// A lazy paragraph continuation still carries the list's content column.
#[test]
fn normalizes_break_after_lazy_list_continuation() {
    let input = lines_vec!["- item", "lazy continuation", "---"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[1], "lazy continuation");
    assert_borrowed_break!(output[2]);
}

/// An indented item continuation remains in the list after a blank line.
#[test]
fn normalizes_break_after_blank_list_continuation() {
    let input = lines_vec!["- item", "", "  continuation", "---"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[2], "  continuation");
    assert_borrowed_break!(output[3]);
}

/// A link definition is a block start, not Setext heading text.
#[test]
fn normalizes_break_after_link_definition() {
    let input = lines_vec!["[a]: /url", "---"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], "[a]: /url");
    assert_borrowed_break!(output[1]);
}

/// An outdented break ends the list rather than underlining its link-shaped line.
#[rstest]
#[case(vec!["- item", "[a]: /url", "---"], 2)]
#[case(vec!["- item", "  [a]: /url", "---"], 2)]
fn normalizes_break_below_outdented_list_item_link(
    #[case] source: Vec<&str>,
    #[case] index: usize,
) {
    let input = source.iter().map(ToString::to_string).collect::<Vec<_>>();
    let output = format_breaks(&input);

    assert_borrowed_value!(output[index - 1], source[index - 1]);
    assert_borrowed_break!(output[index]);
}

/// A link-shaped line after paragraph text remains part of that paragraph.
#[test]
fn preserves_setext_after_paragraph_link_shape() {
    let input = lines_vec!["Title", "[a]: /url", "---"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[2], "---");
    assert!(std::ptr::eq(output[2].as_ref(), input[2].as_str()));
}

/// A link-shaped line continues a list item's paragraph under its own indent.
#[rstest]
#[case(vec!["- item", "  [a]: /url", "  ---"], 2)]
#[case(vec!["- item", "[a]: /url", "  ---"], 2)]
fn preserves_setext_after_list_item_link_shape(#[case] source: Vec<&str>, #[case] index: usize) {
    let input = source.iter().map(ToString::to_string).collect::<Vec<_>>();
    let output = format_breaks(&input);

    assert_borrowed_value!(output[index], "  ---");
    assert!(std::ptr::eq(output[index].as_ref(), input[index].as_str()));
}

/// Underlines at a list item's content column stay inside that item.
#[rstest]
#[case(vec!["- Bar", "  ---"], 1)]
#[case(vec!["- item", "   continuation", "  ---"], 2)]
fn preserves_list_content_underlines(#[case] source: Vec<&str>, #[case] underline_index: usize) {
    let input = source.iter().map(ToString::to_string).collect::<Vec<_>>();
    let output = format_breaks(&input);

    assert_borrowed_value!(output[underline_index], "  ---");
    assert!(std::ptr::eq(
        output[underline_index].as_ref(),
        input[underline_index].as_str()
    ));
}

#[test]
fn test_format_breaks_ignores_code() {
    let input = lines_vec!["```", "---", "```"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], "```");
    assert_borrowed_value!(output[1], "---");
    assert_borrowed_value!(output[2], "```");
}

#[test]
fn test_format_breaks_mixed_chars() {
    let input = lines_vec!["-*-*-"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], "-*-*-");
}

#[test]
fn test_format_breaks_with_spaces_and_indent() {
    let input = lines_vec!["  -  -  -  "];
    let output = format_breaks(&input);

    assert_borrowed_break!(output[0]);
}

/// Leaves a tab-prefixed apparent break untouched as indented code.
#[test]
fn leaves_tab_prefixed_break_as_indented_code() {
    let input = lines_vec!["\t_\t_\t_\t"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], "\t_\t_\t_\t");
}

#[test]
fn test_format_breaks_mixed_chars_excessive_length() {
    let input = lines_vec!["***---___"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], "***---___");
}

/// Tests the CLI `--breaks` option to ensure thematic breaks are normalized.
///
/// Provides a single line of hyphens and asserts the output is the standard
/// underscore-based thematic break.
#[test]
fn test_cli_breaks_option() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--breaks")
        .write_stdin("---\n")
        .assert()
        .success()
        .stdout(format!("{}\n", "_".repeat(THEMATIC_BREAK_LEN)));
}

/// Returns `format_breaks` output as owned lines, for whole-document comparisons.
fn formatted(lines: &[String]) -> Vec<String> {
    format_breaks(lines)
        .into_iter()
        .map(std::borrow::Cow::into_owned)
        .collect()
}

/// Returns the canonical break behind `indent`.
fn indented_break(indent: &str) -> String { format!("{indent}{}", "_".repeat(THEMATIC_BREAK_LEN)) }

/// Regression cases for issue #572: a thematic break inside a list item keeps
/// its indentation and stays in the item.
///
/// A break indented to an item's content column is a child block of that
/// item. Canonicalising it at column 0 took it, and the item's remaining
/// content, out of the list, and split the list in two. Only the break's
/// characters are canonical; its indentation is structural.
#[rstest]
#[case::after_a_blank_line(include_lines!("data/issue_572_break_in_item_input.txt"), 4, "   ")]
#[case::in_a_bullet_item(lines_vec!["- item", "", "  ***", "", "  more"], 2, "  ")]
// A heading between the item's marker and the break is a child block of the
// item; it must not close the item.
#[case::after_a_child_heading(lines_vec!["- item", "", "  # heading", "", "  ***", "", "  more"], 4, "  ")]
// A nested list inside the item closes before the break; the outer item holds it.
#[case::after_a_nested_list(lines_vec!["- a", "  - b", "", "  ***", "", "  more"], 3, "  ")]
// A lazy continuation line at column 0 still belongs to the item's paragraph,
// so it does not close the item.
#[case::after_a_lazy_line(lines_vec!["- item", "lazy continuation", "", "  ***", "", "  more"], 3, "  ")]
// A `10.` item's content starts at column 4; relative to the item this is a
// break, not indented code.
// A fenced block is a child of the item too, so the item stays open across it.
#[case::after_a_fenced_block(lines_vec!["- item", "", "  ```", "  code", "  ```", "", "  ***", "", "  more"], 6, "  ")]
#[case::under_a_wide_marker(lines_vec!["10. item", "", "    ***", "", "    more"], 2, "    ")]
fn breaks_issue_572_break_in_item_keeps_its_indentation(
    #[case] input: Vec<String>,
    #[case] break_index: usize,
    #[case] indent: &str,
) {
    let once = formatted(&input);
    let mut expected = input.clone();
    expected[break_index] = indented_break(indent);
    assert_eq!(once, expected);
    assert_eq!(formatted(&once), once, "a second pass changes nothing");
}

/// Regression cases for issue #572, the neighbouring shapes: a break that is
/// not inside an item is still canonicalised at column 0.
///
/// Indentation of up to three spaces outside an item changes nothing
/// structural, and a break left of an item's content column has already left
/// the item, so both keep the column-0 canonical form.
#[rstest]
#[case::top_level_indent(lines_vec!["text", "", "  ***"], 2)]
#[case::left_of_the_content_column(lines_vec!["1. item", "", "  ***"], 2)]
// A quote at column 0 leaves the item, so the break after it is outside.
#[case::after_an_outdented_quote(lines_vec!["- item", "", "> quote", "", "  ***"], 4)]
// An empty marker, or one that opens a heading or fence, starts no paragraph,
// so the column-0 line after it is not a lazy continuation of the item.
#[case::after_an_empty_marker(lines_vec!["- ", "para", "", "  ***"], 3)]
#[case::after_a_heading_marker(lines_vec!["- # heading", "para", "", "  ***"], 3)]
#[case::after_the_list_ends(lines_vec!["1. item", "", "para", "", "   ***"], 4)]
fn breaks_issue_572_break_outside_an_item_is_emitted_at_column_zero(
    #[case] input: Vec<String>,
    #[case] break_index: usize,
) {
    let once = formatted(&input);
    assert_eq!(once[break_index], indented_break(""));
    assert_eq!(formatted(&once), once, "a second pass changes nothing");
}

/// Regression case for issue #572 through the CLI: `--breaks` keeps the
/// reproduction's break inside its item.
#[test]
fn breaks_issue_572_cli_keeps_the_break_in_the_item() {
    let input = include_str!("data/issue_572_break_in_item_input.txt");
    let expected = input.replace("   ***", &indented_break("   "));
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--breaks")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(expected);
}
