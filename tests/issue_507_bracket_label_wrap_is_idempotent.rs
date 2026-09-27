//! Regression tests for issue #507.
//!
//! Invariant: wrapping a bracket label is idempotent.
//!
//! `--wrap` used to couple a `[` opener to its label only when the label was
//! a run of ASCII digits (#504), so a short alphabetic label such as `[a]`
//! could be split after the opener. The first pass then ended a line with
//! `[`, and the second pass rejoined the pair as `[ a]`. Since #561, a line
//! may only break where the source has whitespace, so a label written without
//! a space after its opener can no longer be split, whatever its characters.
//!
//! The corpus fixtures under `tests/data/idempotence/` are the issue's two
//! reproductions: the label after a space, and the label touching the prose
//! before it. Each is formatted twice through the real binary. The property
//! grows prose to the wrap boundary in front of a generated label, which is
//! where the opener used to be stranded.

use std::path::Path;

use assert_cmd::Command;
use proptest::prelude::*;
use rstest::rstest;

/// Formats `input` with `--wrap` through the binary and returns stdout.
fn wrap(input: &str) -> String {
    let output = Command::cargo_bin("mdtablefix")
        .expect("the mdtablefix binary builds")
        .arg("--wrap")
        .write_stdin(input)
        .output()
        .expect("mdtablefix runs");
    assert!(output.status.success(), "mdtablefix failed: {output:?}");
    String::from_utf8(output.stdout).expect("mdtablefix writes UTF-8")
}

/// Scenario: each reproduction from the issue.
///
/// Invariant: the first pass is a fixed point and keeps `[a]` whole.
#[rstest]
#[case::label_after_space("issue_507_label_after_space.dat")]
#[case::label_touching_prose("issue_507_label_touching.dat")]
fn a_bracket_label_is_a_fixed_point(#[case] fixture: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/idempotence")
        .join(fixture);
    let input = std::fs::read_to_string(&path).expect("the fixture is readable");
    let first = wrap(&input);
    assert!(first.contains("[a]"), "the label was split: {first:?}");
    assert!(
        first.lines().all(|line| !line.ends_with('[')),
        "an opener was stranded at a line end: {first:?}"
    );
    assert_eq!(wrap(&first), first, "the second pass changed the output");
}

proptest! {
    /// A label of letters, digits or both, written with or without a space
    /// after the prose, sitting on the wrap boundary, is kept whole and is a
    /// fixed point.
    #[test]
    fn any_bracket_label_on_the_boundary_is_a_fixed_point(
        label in "[a-z0-9]{1,4}",
        head in 76_usize..80,
        spaced in any::<bool>(),
    ) {
        let separator = if spaced { " " } else { "" };
        let input = format!("{}{separator}[{label}] tail.\n", "a".repeat(head));
        let first = wrap(&input);
        let whole = format!("[{label}]");
        prop_assert!(first.contains(&whole), "{whole} split: {first:?}");
        prop_assert_eq!(wrap(&first), first);
    }
}
