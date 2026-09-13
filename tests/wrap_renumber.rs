//! Regression test for combined wrapping and renumbering.

use mdtablefix::process_stream;

#[macro_use]
#[path = "common/mod.rs"]
mod common;

#[test]
fn wrap_then_renumber_preserves_order() {
    let input: Vec<String> = include_lines!("data/wrap_renumber_regression_input.txt");
    let expected: Vec<String> = include_lines!("data/wrap_renumber_regression_expected.txt");

    let out = process_stream(&input);

    assert_eq!(
        out, expected,
        "renumbered output mismatch:\nexpected: {expected:?}\nactual: {out:?}",
    );
}

#[test]
fn wrap_then_renumber_preserves_inline_code_items() {
    let input: Vec<String> = include_lines!("data/wrap_renumber_inline_code_input.txt");
    let expected: Vec<String> = include_lines!("data/wrap_renumber_inline_code_expected.txt");

    let out = process_stream(&input);

    assert_eq!(
        out, expected,
        "inline-code list output mismatch:\nexpected: {expected:?}\nactual: {out:?}",
    );
}

#[test]
fn wrap_then_renumber_preserves_leading_code_span() {
    let input: Vec<String> = include_lines!("data/wrap_renumber_leading_code_input.txt");
    let expected: Vec<String> = include_lines!("data/wrap_renumber_leading_code_expected.txt");

    let out = process_stream(&input);

    assert_eq!(
        out, expected,
        "leading-code list output mismatch:\nexpected: {expected:?}\nactual: {out:?}",
    );
}
