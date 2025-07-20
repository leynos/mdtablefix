//! Integration tests for footnote conversion.

use mdtablefix::convert_footnotes;

#[macro_use]
mod prelude;

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
