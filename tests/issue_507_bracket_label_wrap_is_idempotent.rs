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

use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;
use rstest::rstest;

#[path = "support/cli_stdin.rs"]
mod cli_stdin;

use cli_stdin::run_cli_with_stdin;

/// Formats `input` with `--wrap` through the binary and returns stdout.
///
/// The launch and status handling belong to `cli_stdin`; this only decodes.
fn wrap(input: &str) -> String {
    let assert = run_cli_with_stdin(&["--wrap"], input).expect("mdtablefix builds and runs");
    let output = assert.success().get_output().stdout.clone();
    String::from_utf8(output).expect("mdtablefix writes UTF-8")
}

/// Reads a corpus fixture through a directory capability rather than the
/// ambient filesystem.
fn fixture(name: &str) -> String {
    Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .and_then(|root| root.read_to_string(format!("tests/data/idempotence/{name}")))
        .expect("the fixture is readable")
}

/// Scenario: the label written after a space, at the wrap boundary.
///
/// Invariant: the label moves whole to the next line. Before the fix the
/// opener was stranded at the end of the first line, so the exact output
/// distinguishes fixed from broken; a bare `contains("[a]")` does not.
#[test]
fn a_label_after_a_space_moves_whole_to_the_next_line() {
    let input = fixture("issue_507_label_after_space.dat");
    let head = input
        .trim_end()
        .strip_suffix(" [a]")
        .expect("the fixture ends with a spaced label");
    let first = wrap(&input);
    assert_eq!(first, format!("{head}\n[a]\n"), "the label was split");
    assert_eq!(wrap(&first), first, "the second pass changed the output");
}

/// Scenario: the label touching the prose before it, past the boundary.
///
/// Invariant: with no whitespace to break at, the line is left as written.
#[test]
fn a_label_touching_prose_is_left_on_its_line() {
    let input = fixture("issue_507_label_touching.dat");
    let first = wrap(&input);
    assert_eq!(first, input, "a line with no break opportunity was split");
    assert_eq!(wrap(&first), first, "the second pass changed the output");
}

/// Scenario: an opener followed by a space, on the wrap boundary.
///
/// Invariant: the space is a genuine break opportunity, so the line may end
/// with the opener, and doing so is stable. This is the narrowing of #570's
/// fix: only touching tokens are bound.
#[rstest]
#[case::breaks_after_the_opener(77)]
fn an_opener_followed_by_a_space_keeps_its_break_opportunity(#[case] head: usize) {
    let prose = "a".repeat(head);
    let first = wrap(&format!("{prose} [ a] tail.\n"));
    assert_eq!(
        first,
        format!("{prose} [\na] tail.\n"),
        "no break after `[`"
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
        prop_assert!(
            first.lines().all(|line| !line.ends_with('[')),
            "an opener was stranded at a line end: {first:?}"
        );
        prop_assert_eq!(wrap(&first), first);
    }
}
