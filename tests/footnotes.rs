//! Integration tests for footnote conversion.
//!
//! These tests feed entire documents through `convert_footnotes` to validate
//! the complete conversion pipeline. Sample inputs are loaded from data files
//! using the helper macros `include_lines!` and `lines_vec!` exported by the
//! `tests::prelude` module. The goal is to ensure inline references are
//! rewritten and trailing numeric lists become footnote definitions without
//! affecting other content.
//!
//! The presence of the macros here also confirms they are re-exported
//! correctly for use across integration tests.

use mdtablefix::convert_footnotes;

#[macro_use]
mod prelude;

#[test]
fn macros_available() {
    let _: Vec<String> = lines_vec!("a", "b");
    let _: Vec<String> = include_lines!("data/footnotes_input.txt");
}

#[test]
fn test_convert_bare_footnotes() {
    let input: Vec<String> = include_lines!("data/footnotes_input.txt");
    let expected: Vec<String> = include_lines!("data/footnotes_expected.txt");
    let output = convert_footnotes(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_idempotent_on_converted() {
    let expected: Vec<String> = include_lines!("data/footnotes_expected.txt");
    let output = convert_footnotes(&expected);
    assert_eq!(output, expected);
}

#[test]
fn test_avoids_false_positives() {
    let input = lines_vec!("Plan9 is interesting.", "Call 1-800-555-1234 for help.",);
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_ignores_numbers_in_inline_code() {
    let input = lines_vec!("Look at `code 1` for details.");
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_ignores_numbers_in_parentheses() {
    let input = lines_vec!("Refer to equation (1) for context.");
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_ignores_numbers_in_fenced_code_block() {
    let input = lines_vec!(
        "Here is a code block:",
        "```",
        "let x = 42; // note 1",
        "```",
        "Done."
    );
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_ignores_numbers_in_indented_code_block() {
    let input = lines_vec!(
        "    let a = 1;",
        "    let b = 2; // number 2",
        "",
        "Outside."
    );
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_handles_punctuation_inside_bold() {
    let input = lines_vec!("It was **scary.**7");
    let expected = lines_vec!("It was **scary.**[^7]");
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn test_empty_input() {
    let input: Vec<String> = Vec::new();
    let output = convert_footnotes(&input);
    assert!(output.is_empty());
}

#[test]
fn test_whitespace_input() {
    let input = lines_vec!("   ", "\t");
    let output = convert_footnotes(&input);
    assert_eq!(output, input);
}
