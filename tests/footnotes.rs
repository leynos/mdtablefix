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
