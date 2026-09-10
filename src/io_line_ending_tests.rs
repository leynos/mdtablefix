//! Unit tests for line-ending detection and serialization.
//!
//! These moved out of `io.rs` so that adding them did not push the production
//! module past the 400-line limit AGENTS.md sets.

use std::{fs, path::Path};

use proptest::prelude::*;
use rstest::rstest;
use tempfile::tempdir;

use super::*;

/// Writes `input` to a temporary file, rewrites it with `rewrite_fn`, and
/// returns the file's bytes.
fn rewritten_bytes(input: &str, rewrite_fn: fn(&Path) -> std::io::Result<()>) -> Vec<u8> {
    let dir = tempdir().expect("create temporary directory");
    let file = dir.path().join("sample.md");
    fs::write(&file, input).expect("write fixture");
    rewrite_fn(&file).expect("rewrite fixture");
    fs::read(&file).expect("read rewritten fixture")
}

#[rstest]
#[case::no_line_endings("alpha", LineEnding::Lf)]
#[case::line_feeds_only("alpha\nbeta\n", LineEnding::Lf)]
#[case::carriage_returns_only("alpha\r\nbeta\r\n", LineEnding::Crlf)]
#[case::carriage_return_majority("alpha\r\nbeta\r\ngamma\n", LineEnding::Crlf)]
#[case::line_feed_majority("alpha\nbeta\ngamma\r\n", LineEnding::Lf)]
#[case::exact_tie("alpha\r\nbeta\n", LineEnding::Lf)]
fn detect_line_ending_selects_the_majority_style(#[case] text: &str, #[case] expected: LineEnding) {
    assert_eq!(detect_line_ending(text), expected);
}

#[test]
fn serialize_lines_uses_the_chosen_ending() {
    let lines = vec!["| A | B |".to_string(), "| 1 | 2 |".to_string()];
    assert_eq!(
        serialize_lines(&lines, LineEnding::Lf),
        "| A | B |\n| 1 | 2 |\n"
    );
    assert_eq!(
        serialize_lines(&lines, LineEnding::Crlf),
        "| A | B |\r\n| 1 | 2 |\r\n"
    );
    assert!(serialize_lines(&[], LineEnding::Crlf).is_empty());
}

/// Byte-exact rewrite cases: input text and the expected file bytes.
///
/// Each case reflows a ragged table, so a passing case proves the output
/// was reformatted and not merely copied.
const REWRITE_LINE_ENDING_CASES: &[(&str, &str)] = &[
    (
        "|A|B|\n|---|---|\n|1|2|\n",
        "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n",
    ),
    (
        "|A|B|\r\n|---|---|\r\n|1|2|\r\n",
        "| A   | B   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n",
    ),
    (
        "|A|B|\r\n|---|---|\r\n|1|2|\n",
        "| A   | B   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n",
    ),
    (
        "|A|B|\n|---|---|\n|1|2|\r\n",
        "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n",
    ),
    ("|A|B|\r\n|---|---|\n", "| A   | B   |\n| --- | --- |\n"),
    ("Only prose", "Only prose\n"),
];

#[rstest]
#[case(rewrite)]
#[case(rewrite_no_wrap)]
fn rewrite_preserves_the_majority_line_ending(
    #[case] rewrite_fn: fn(&Path) -> std::io::Result<()>,
) {
    for (input, expected) in REWRITE_LINE_ENDING_CASES {
        assert_eq!(
            rewritten_bytes(input, rewrite_fn),
            expected.as_bytes(),
            "unexpected bytes for input {input:?}"
        );
    }
}

proptest! {
    /// Any mixture of endings is rewritten to the majority style alone.
    #[test]
    fn rewrite_emits_only_the_majority_ending(
        endings in prop::collection::vec(prop_oneof![Just("\n"), Just("\r\n")], 2..8),
    ) {
        let rows = ["| A | B |", "|---|---|", "| 1 | 2 |", "| 3 | 4 |"];
        let mut input = String::new();
        for (index, ending) in endings.iter().enumerate() {
            input.push_str(rows[index % rows.len()]);
            input.push_str(ending);
        }

        let carriage_returns = endings.iter().filter(|ending| **ending == "\r\n").count();
        let expected = if carriage_returns * 2 > endings.len() { "\r\n" } else { "\n" };
        let output = String::from_utf8(rewritten_bytes(&input, rewrite))
            .expect("rewritten output is UTF-8");

        prop_assert!(
            output.starts_with("| A   | B   |"),
            "table was not reflowed: {output:?}"
        );
        prop_assert!(
            output.ends_with(expected),
            "output does not end with the majority ending: {output:?}"
        );
        let crlf_pairs = output.matches("\r\n").count();
        if expected == "\r\n" {
            prop_assert_eq!(
                output.matches('\n').count() - crlf_pairs,
                0,
                "CRLF output contains bare line feeds: {:?}",
                output
            );
        } else {
            prop_assert_eq!(crlf_pairs, 0, "LF output contains CRLF pairs: {:?}", output);
        }
    }
}
