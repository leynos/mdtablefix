//! Byte-exact tests for majority line-ending preservation.
//!
//! The command-line interface must terminate every output line with the style
//! holding the strict majority of the input's line endings, so a CRLF document
//! is never rewritten as LF. Assertions compare raw bytes, because `String`
//! helpers such as `lines` would hide the difference between `\r\n` and `\n`.

use std::fs;

use rstest::rstest;
use tempfile::tempdir;

#[path = "support/cli_args.rs"]
mod cli_args;
#[path = "support/cli_stdin.rs"]
mod cli_stdin;

use cli_args::run_cli_with_args;
use cli_stdin::run_cli_with_stdin;

/// A named input document and the exact bytes the formatter must emit.
struct LineEndingCase {
    name: &'static str,
    input: &'static str,
    expected: &'static str,
}

/// Consistent styles, both majority styles in a mixed document, the LF
/// tie-break, and a document with no line ending at all.
const CASES: &[LineEndingCase] = &[
    LineEndingCase {
        name: "line_feeds_only",
        input: "| A | B |\n| --- | --- |\n| 1 | 2 |\n",
        expected: "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n",
    },
    LineEndingCase {
        name: "carriage_returns_only",
        input: "| A | B |\r\n| --- | --- |\r\n| 1 | 2 |\r\n",
        expected: "| A   | B   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n",
    },
    LineEndingCase {
        name: "carriage_return_majority",
        input: "| A | B |\r\n| --- | --- |\r\n| 1 | 2 |\n",
        expected: "| A   | B   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n",
    },
    LineEndingCase {
        name: "line_feed_majority",
        input: "| A | B |\n| --- | --- |\n| 1 | 2 |\r\n",
        expected: "| A   | B   |\n| --- | --- |\n| 1   | 2   |\n",
    },
    // One ending of each style: neither holds a majority, and the documented
    // tie-break selects LF regardless of which style appears first.
    LineEndingCase {
        name: "no_majority_defaults_to_line_feeds",
        input: "| A | B |\n| --- | --- |\r\n",
        expected: "| A   | B   |\n| --- | --- |\n",
    },
    LineEndingCase {
        name: "no_line_endings",
        input: "Only prose",
        expected: "Only prose\n",
    },
];

/// Assert the formatter output for `case`, naming the case on failure.
fn assert_case_output(case: &LineEndingCase, actual: &[u8]) {
    let actual = String::from_utf8(actual.to_vec()).expect("formatter output is not UTF-8");
    assert_eq!(
        actual, case.expected,
        "case `{}` produced the wrong bytes",
        case.name
    );
}

#[rstest]
fn stdout_preserves_the_majority_line_ending() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir().expect("failed to create temporary directory");
    for case in CASES {
        let path = dir.path().join(format!("{}.md", case.name));
        fs::write(&path, case.input).expect("failed to write fixture");
        let path = path.to_str().expect("fixture path is not valid UTF-8");
        let output = run_cli_with_args(&[path])?
            .success()
            .get_output()
            .stdout
            .clone();
        assert_case_output(case, &output);
    }
    Ok(())
}

#[rstest]
fn in_place_preserves_the_majority_line_ending() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir().expect("failed to create temporary directory");
    for case in CASES {
        let path = dir.path().join(format!("in-place-{}.md", case.name));
        fs::write(&path, case.input).expect("failed to write fixture");
        let path = path.to_str().expect("fixture path is not valid UTF-8");
        run_cli_with_args(&["--in-place", path])?.success();
        let actual = fs::read(path).expect("failed to read rewritten fixture");
        assert_case_output(case, &actual);
    }
    Ok(())
}

#[rstest]
fn stdin_preserves_the_majority_line_ending() -> Result<(), Box<dyn std::error::Error>> {
    for case in CASES {
        let output = run_cli_with_stdin(&[], case.input)?
            .success()
            .get_output()
            .stdout
            .clone();
        assert_case_output(case, &output);
    }
    Ok(())
}

#[rstest]
fn wrapped_prose_keeps_its_carriage_return_endings() -> Result<(), Box<dyn std::error::Error>> {
    const INPUT: &str = concat!(
        "The formatter must preserve the majority line-ending style of the input ",
        "document so that a file authored on Windows is not rewritten with Unix ",
        "line feeds.\r\n",
    );
    const EXPECTED: &str = concat!(
        "The formatter must preserve the majority line-ending style of the input\r\n",
        "document so that a file authored on Windows is not rewritten with Unix line\r\n",
        "feeds.\r\n",
    );

    let dir = tempdir().expect("failed to create temporary directory");
    let path = dir.path().join("wrapped.md");
    fs::write(&path, INPUT).expect("failed to write fixture");
    let path = path.to_str().expect("fixture path is not valid UTF-8");

    let output = run_cli_with_args(&["--wrap", path])?
        .success()
        .get_output()
        .stdout
        .clone();
    assert_case_output(
        &LineEndingCase {
            name: "wrapped_prose_stdout",
            input: INPUT,
            expected: EXPECTED,
        },
        &output,
    );

    run_cli_with_args(&["--wrap", "--in-place", path])?.success();
    let actual = fs::read(path).expect("failed to read rewritten fixture");
    assert_case_output(
        &LineEndingCase {
            name: "wrapped_prose_in_place",
            input: INPUT,
            expected: EXPECTED,
        },
        &actual,
    );
    Ok(())
}

#[rstest]
fn empty_file_produces_empty_output() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir().expect("failed to create temporary directory");
    let path = dir.path().join("empty.md");
    fs::write(&path, "").expect("failed to write fixture");
    let path = path.to_str().expect("fixture path is not valid UTF-8");

    let output = run_cli_with_args(&[path])?
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(output.is_empty(), "empty file printed {output:?}");

    run_cli_with_args(&["--in-place", path])?.success();
    assert!(
        fs::read(path)
            .expect("failed to read rewritten fixture")
            .is_empty(),
        "in-place rewriting an empty file wrote bytes"
    );
    Ok(())
}

#[rstest]
fn empty_stdin_prints_one_line_feed() -> Result<(), Box<dyn std::error::Error>> {
    // Historical stdin contract: one terminator even when no lines are
    // produced, and LF because the input carries no line ending to detect.
    let output = run_cli_with_stdin(&[], "")?
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(String::from_utf8(output)?, "\n");
    Ok(())
}
