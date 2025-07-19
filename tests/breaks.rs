//! Integration tests for formatting thematic breaks.
//!
//! Verifies `format_breaks` function and `--breaks` CLI option.

use std::borrow::Cow;

use mdtablefix::{THEMATIC_BREAK_LEN, format_breaks};

mod prelude;
use prelude::*;

#[test]
fn test_format_breaks_basic() {
    let input = lines_vec!["foo", "***", "bar"];
    let expected: Vec<Cow<str>> = vec![
        input[0].as_str().into(),
        Cow::Owned("_".repeat(THEMATIC_BREAK_LEN)),
        input[2].as_str().into(),
    ];
    assert_eq!(format_breaks(&input), expected);
}

#[test]
fn test_format_breaks_ignores_code() {
    let input = lines_vec!["```", "---", "```"];
    let expected: Vec<Cow<str>> = input.iter().map(|s| s.as_str().into()).collect();
    assert_eq!(format_breaks(&input), expected);
}

#[test]
fn test_format_breaks_mixed_chars() {
    let input = lines_vec!["-*-*-"];
    let expected: Vec<Cow<str>> = input.iter().map(|s| s.as_str().into()).collect();
    assert_eq!(format_breaks(&input), expected);
}

#[test]
fn test_format_breaks_with_spaces_and_indent() {
    let input = lines_vec!["  -  -  -  "];
    let expected: Vec<Cow<str>> = vec![Cow::Owned("_".repeat(THEMATIC_BREAK_LEN))];
    assert_eq!(format_breaks(&input), expected);
}

#[test]
fn test_format_breaks_with_tabs_and_underscores() {
    let input = lines_vec!["\t_\t_\t_\t"];
    let expected: Vec<Cow<str>> = vec![Cow::Owned("_".repeat(THEMATIC_BREAK_LEN))];
    assert_eq!(format_breaks(&input), expected);
}

#[test]
fn test_format_breaks_mixed_chars_excessive_length() {
    let input = lines_vec!["***---___"];
    let expected: Vec<Cow<str>> = input.iter().map(|s| s.as_str().into()).collect();
    assert_eq!(format_breaks(&input), expected);
}

/// Tests the CLI `--breaks` option to ensure thematic breaks are normalised.
///
/// Provides a single line of hyphens and asserts the output is the standard
/// underscore-based thematic break.
#[test]
fn test_cli_breaks_option() {
    let output = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--breaks")
        .write_stdin("---\n")
        .output()
        .expect("Failed to execute mdtablefix command");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{}\n", "_".repeat(THEMATIC_BREAK_LEN))
    );
}
