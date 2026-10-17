//! CLI integration tests for Setext heading conversion and block boundaries.

use assert_cmd::Command;
use rstest::rstest;

/// Tests that the `--headings` option converts Setext headings to ATX headings.
#[test]
fn test_cli_headings_option() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--headings")
        .write_stdin("Title\n=====\n")
        .assert()
        .success()
        .stdout("# Title\n");
}

/// Verifies that Setext headings are left untouched unless `--headings` is provided.
#[test]
fn test_cli_headings_disabled_by_default() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .write_stdin("Heading\n-----\n")
        .assert()
        .success()
        .stdout("Heading\n-----\n");
}

/// Ensures the `--headings` option ignores short underline markers to avoid false positives.
#[test]
fn test_cli_headings_requires_long_marker() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--headings")
        .write_stdin("Maybe not\n==\n")
        .assert()
        .success()
        .stdout("Maybe not\n==\n");
}

/// Ensures blockquote paragraphs are not turned into headings when underline markers lack the
/// corresponding quote prefix.
#[test]
fn test_cli_headings_preserves_blockquote_paragraphs() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--headings")
        .write_stdin("> Quote\n-----\n")
        .assert()
        .success()
        .stdout("> Quote\n-----\n");
}

/// Ensures the `--headings` option rewrites blockquote headings while keeping
/// the quote prefix.
#[test]
fn test_cli_headings_blockquote_conversion() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--headings")
        .write_stdin("> Quote\n> ----\n")
        .assert()
        .success()
        .stdout("> ## Quote\n");
}

/// Ensures a candidate that is itself a block start does not become a heading.
///
/// `--headings` consumes the following `---`/`===` line as an underline, so a
/// candidate that is already a block would swallow the line below it: `## aa`
/// above `---` produced the single line `## ## aa`, and the break was lost.
/// Every case must therefore survive unchanged.
#[rstest]
#[case("## aa\n---\n")]
#[case("# aa\n===\n")]
#[case("###### aa ######\n---\n")]
#[case("> ## aa\n> ---\n")]
#[case("---\n---\n")]
#[case("***\n---\n")]
#[case("- item\n---\n")]
#[case("1. item\n---\n")]
#[case("  - item\n  ---\n")]
#[case("[^1]: note\n---\n")]
#[case("[label]: https://example.com\n---\n")]
#[case("<!-- markdownlint-disable MD013 -->\n---\n")]
fn test_cli_headings_preserves_block_starts(#[case] input: &'static str) {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--headings")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(input);
}
