//! Integration tests for formatting thematic breaks.
//!
//! Verifies `format_breaks` function and `--breaks` CLI option.

use assert_cmd::Command;
use mdtablefix::{THEMATIC_BREAK_LEN, format_breaks};

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

#[test]
fn test_format_breaks_with_tabs_and_underscores() {
    let input = lines_vec!["\t_\t_\t_\t"];
    let output = format_breaks(&input);

    assert_borrowed_break!(output[0]);
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
