//! Regression test for combined wrapping and renumbering.

use mdtablefix::{process_stream, renumber_lists};

#[macro_use]
mod prelude;

#[test]
fn wrap_then_renumber_preserves_order() {
    let input: Vec<String> = include_lines!("data/wrap_renumber_regression_input.txt");
    let expected: Vec<String> = include_lines!("data/wrap_renumber_regression_expected.txt");

    let mut out = process_stream(&input);
    out = renumber_lists(&out);

    assert_eq!(
        out,
        expected,
        "renumbered output mismatch:\nexpected: {expected:?}\nactual: {out:?}",
        expected = expected,
        out = out,
    );
}
