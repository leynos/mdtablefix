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

/// A link definition is a block start, not Setext heading text.
#[test]
fn normalizes_break_after_link_definition() {
    let input = lines_vec!["[a]: /url", "---"];
    let output = format_breaks(&input);

    assert_borrowed_value!(output[0], "[a]: /url");
    assert_borrowed_break!(output[1]);
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
