//! Regression test for combined wrapping and renumbering.

use mdtablefix::{Options, process_stream, process_stream_opts};

#[macro_use]
#[path = "common/mod.rs"]
mod common;

fn wrap_and_renumber(input: &[String]) -> Vec<String> {
    process_stream_opts(
        input,
        Options {
            wrap: true,
            renumber: true,
            ..Default::default()
        },
    )
}

#[test]
fn process_stream_keeps_deliberate_ordered_list_markers() {
    let input = vec!["5. Deliberate start number".to_string()];

    assert_eq!(process_stream(&input), input);
}

#[test]
fn wrap_then_renumber_preserves_order() {
    let input: Vec<String> = include_lines!("data/wrap_renumber_regression_input.txt");
    let expected: Vec<String> = include_lines!("data/wrap_renumber_regression_expected.txt");

    let out = wrap_and_renumber(&input);

    assert_eq!(
        out, expected,
        "renumbered output mismatch:\nexpected: {expected:?}\nactual: {out:?}",
    );
}

#[test]
fn wrap_then_renumber_preserves_inline_code_items() {
    let input: Vec<String> = include_lines!("data/wrap_renumber_inline_code_input.txt");
    let expected: Vec<String> = include_lines!("data/wrap_renumber_inline_code_expected.txt");

    let out = wrap_and_renumber(&input);

    assert_eq!(
        out, expected,
        "inline-code list output mismatch:\nexpected: {expected:?}\nactual: {out:?}",
    );
}

#[test]
fn wrap_then_renumber_preserves_leading_code_span() {
    let input: Vec<String> = include_lines!("data/wrap_renumber_leading_code_input.txt");
    let expected: Vec<String> = include_lines!("data/wrap_renumber_leading_code_expected.txt");

    let out = wrap_and_renumber(&input);

    assert_eq!(
        out, expected,
        "leading-code list output mismatch:\nexpected: {expected:?}\nactual: {out:?}",
    );
}
