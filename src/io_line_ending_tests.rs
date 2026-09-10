//! Unit tests for line-ending detection, serialization, and the event the
//! rewrite boundary emits.
//!
//! These moved out of `io.rs` so that adding them did not push the production
//! module past the 400-line limit AGENTS.md sets.

use std::{fs, path::Path};

use proptest::prelude::*;
use rstest::rstest;
use tempfile::tempdir;
use tracing_test::traced_test;

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

/// The counts behind a decision are available without restating the rule.
#[rstest]
#[case::no_line_endings("alpha", LineEnding::Lf, 0, 0)]
#[case::line_feeds_only("alpha\nbeta\n", LineEnding::Lf, 0, 2)]
#[case::carriage_returns_only("alpha\r\nbeta\r\n", LineEnding::Crlf, 2, 0)]
#[case::carriage_return_majority("alpha\r\nbeta\r\ngamma\n", LineEnding::Crlf, 2, 1)]
#[case::line_feed_majority("alpha\nbeta\ngamma\r\n", LineEnding::Lf, 1, 2)]
#[case::exact_tie("alpha\r\nbeta\n", LineEnding::Lf, 1, 1)]
fn count_line_endings_reports_the_vote(
    #[case] text: &str,
    #[case] ending: LineEnding,
    #[case] crlf_count: usize,
    #[case] lone_lf_count: usize,
) {
    assert_eq!(
        count_line_endings(text),
        LineEndingCounts {
            ending,
            crlf_count,
            lone_lf_count
        },
        "unexpected counts for {text:?}"
    );
}

/// Byte-exact rewrite cases: the input text, and the file bytes expected
/// after the rewrite.
///
/// Each case reflows a ragged table, so a passing case proves the output
/// was reformatted and not merely copied.
#[rstest]
#[case::rewrite_line_feeds(
    rewrite,
    "|A|B|\n|---|---|\n|1|2|\n",
    "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n"
)]
#[case::rewrite_carriage_returns(
    rewrite,
    "|A|B|\r\n|---|---|\r\n|1|2|\r\n",
    "| A   | B   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n"
)]
#[case::rewrite_carriage_return_majority(
    rewrite,
    "|A|B|\r\n|---|---|\r\n|1|2|\n",
    "| A   | B   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n"
)]
#[case::rewrite_line_feed_majority(
    rewrite,
    "|A|B|\n|---|---|\n|1|2|\r\n",
    "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n"
)]
#[case::rewrite_exact_tie(rewrite, "|A|B|\r\n|---|---|\n", "| A   | B   |\n| --- | --- |\n")]
#[case::rewrite_no_line_endings(rewrite, "Only prose", "Only prose\n")]
#[case::no_wrap_line_feeds(
    rewrite_no_wrap,
    "|A|B|\n|---|---|\n|1|2|\n",
    "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n"
)]
#[case::no_wrap_carriage_returns(
    rewrite_no_wrap,
    "|A|B|\r\n|---|---|\r\n|1|2|\r\n",
    "| A   | B   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n"
)]
#[case::no_wrap_carriage_return_majority(
    rewrite_no_wrap,
    "|A|B|\r\n|---|---|\r\n|1|2|\n",
    "| A   | B   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n"
)]
#[case::no_wrap_line_feed_majority(
    rewrite_no_wrap,
    "|A|B|\n|---|---|\n|1|2|\r\n",
    "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n"
)]
#[case::no_wrap_exact_tie(
    rewrite_no_wrap,
    "|A|B|\r\n|---|---|\n",
    "| A   | B   |\n| --- | --- |\n"
)]
#[case::no_wrap_no_line_endings(rewrite_no_wrap, "Only prose", "Only prose\n")]
fn rewrite_preserves_the_majority_line_ending(
    #[case] rewrite_fn: fn(&Path) -> std::io::Result<()>,
    #[case] input: &str,
    #[case] expected: &str,
) {
    assert_eq!(
        rewritten_bytes(input, rewrite_fn),
        expected.as_bytes(),
        "unexpected bytes for input {input:?}"
    );
}

/// The shared helper reports the vote at every boundary: the library
/// rewrite, which adds no boundary fields, and the executable's standard
/// input and file boundaries, which add `operation` and, for files, `path`.
#[test]
#[traced_test]
fn count_line_endings_reported_covers_every_boundary() {
    let text = "alpha\r\nbeta\r\n";
    let library = count_line_endings_reported(text, None, None);
    let stdin = count_line_endings_reported(text, Some("stdin"), None);
    let file = count_line_endings_reported(text, Some("file"), Some("doc.md"));

    for counts in [library, stdin, file] {
        assert_eq!(counts.ending, LineEnding::Crlf);
        assert_eq!(counts.crlf_count, 2);
        assert_eq!(counts.lone_lf_count, 0);
    }
    logs_assert(|lines| {
        let reports: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|line| line.contains("selected the majority line ending"))
            .collect();
        let has = |needle: &str| reports.iter().any(|line| line.contains(needle));
        let unnamed = reports.iter().any(|line| !line.contains("operation="));
        if reports.len() == 3
            && unnamed
            && has("crlf_count=2")
            && has("lone_lf_count=0")
            && has(r#"operation="stdin""#)
            && has(r#"operation="file""#)
            && has("path=doc.md")
        {
            Ok(())
        } else {
            Err(format!("unexpected line-ending reports: {lines:?}"))
        }
    });
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

/// The majority rule stays a pure query: callers can ask which ending a
/// document would select without emitting diagnostics.
#[test]
#[traced_test]
fn detect_line_ending_emits_nothing() {
    assert_eq!(detect_line_ending("alpha\r\nbeta\r\n"), LineEnding::Crlf);
    assert!(
        !logs_contain("selected the majority line ending"),
        "the pure query emitted a diagnostic event"
    );
}

/// The rewrite boundary reports the decision, with the counts behind it, so
/// a rewritten file's endings are traceable.
#[test]
#[traced_test]
fn rewrite_reports_the_selected_ending() {
    let dir = tempdir().expect("create temporary directory");
    let file = dir.path().join("reported.md");
    fs::write(&file, "|A|B|\r\n|---|---|\r\n|1|2|\r\n").expect("write fixture");
    rewrite(&file).expect("rewrite fixture");
    logs_assert(|lines| {
        let reported = lines
            .iter()
            .find(|line| line.contains("selected the majority line ending"));
        match reported {
            Some(line)
                if line.contains("crlf_count=3")
                    && line.contains("lone_lf_count=0")
                    && line.contains(r#"selected_ending="\r\n""#) =>
            {
                Ok(())
            }
            _ => Err(format!("unexpected line-ending report: {lines:?}")),
        }
    });
}
