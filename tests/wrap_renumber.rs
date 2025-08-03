//! Regression test for combined wrapping and renumbering.

use mdtablefix::{process_stream, renumber_lists};
use std::fs;

#[macro_use]
mod prelude;

// File paths for the regression fixtures to avoid repetition.
const INPUT_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/wrap_renumber_regression_input.txt",
);
const EXPECTED_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/wrap_renumber_regression_expected.txt",
);

#[test]
fn wrap_then_renumber_preserves_order() {
    let input = fs::read_to_string(INPUT_FILE).expect("read regression input");
    let expected = fs::read_to_string(EXPECTED_FILE).expect("read regression output");
    let input: Vec<String> = input.lines().map(str::to_owned).collect();
    let expected: Vec<String> = expected.lines().map(str::to_owned).collect();

    let mut out = process_stream(&input);
    out = renumber_lists(&out);

    assert_eq!(
        out, expected,
        "processed output differed\nexpected: {expected:?}\nactual: {out:?}",
    );
}
