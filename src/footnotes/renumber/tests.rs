//! Unit tests for footnote renumbering helpers.
//!
//! These cases exercise the private numeric-candidate parsing and renumbering
//! helpers directly so the behaviour stays covered without bloating the main
//! module file.

use rstest::rstest;

use super::{numeric_candidate_from_line, renumber_footnotes};

fn strings(lines: &[&str]) -> Vec<String> { lines.iter().map(|line| (*line).to_string()).collect() }

#[rstest]
#[case("7.")]
#[case("7:")]
fn malformed_numeric_candidate_line_is_ignored(#[case] line: &str) {
    assert!(numeric_candidate_from_line(line, 0).is_none());
}

#[test]
fn renumber_footnotes_rewrites_existing_definition_headers() {
    let mut lines = strings(&["Reference.[^7]", "", "[^7]: Existing definition"]);

    renumber_footnotes(&mut lines);

    assert_eq!(
        lines,
        strings(&["Reference.[^1]", "", "[^1]: Existing definition"])
    );
}

#[test]
fn renumber_footnotes_rewrites_numeric_candidates() {
    let mut lines = strings(&["Reference.[^7]", "", "7. Legacy footnote"]);

    renumber_footnotes(&mut lines);

    assert_eq!(
        lines,
        strings(&["Reference.[^1]", "", "[^1]: Legacy footnote"])
    );
}
