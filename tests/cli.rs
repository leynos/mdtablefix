//! Integration tests for CLI interface behaviour of the `mdtablefix` tool.
//!
//! This module validates the command-line interface functionality, including:
//! - File handling with the `--in-place` flag
//! - Ellipsis replacement with the `--ellipsis` option
//! - Error handling for invalid argument combinations
//! - Processing of Markdown files through the CLI interface

use assert_cmd::Command;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::rstest;
use tempfile::{TempDir, tempdir};

#[macro_use]
#[path = "common/mod.rs"]
mod common;
#[path = "cli/ellipsis.rs"]
mod ellipsis;
#[path = "support/fixtures.rs"]
mod fixtures;
use fixtures::broken_table;

/// Verifies that the CLI fails when the `--in-place` flag is used without specifying a file.
///
/// This test ensures that running `mdtablefix --in-place` without a file argument results in a
/// command failure.
#[test]
fn test_cli_in_place_requires_file() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--in-place")
        .assert()
        .failure();
}

/// Verifies that the `--version` flag prints the crate version and exits.
#[test]
fn test_cli_version_flag() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--version")
        .assert()
        .success()
        .stdout(format!("mdtablefix {}\n", env!("CARGO_PKG_VERSION")));
}

/// Tests that the CLI processes a file containing a broken Markdown table and outputs the corrected
/// table to stdout.
///
/// This test creates a temporary file with a malformed table, runs the `mdtablefix` binary on it,
/// and asserts that the output is the expected fixed table.
#[rstest]
fn test_cli_process_file(broken_table: Vec<String>) {
    let dir = tempdir().expect("failed to create temporary directory");
    let (directory, parent_path) = capability_directory(&dir);
    let file_name = Utf8Path::new("sample.md");
    directory
        .write(file_name, format!("{}\n", broken_table.join("\n")))
        .expect("failed to write temporary file");
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg(parent_path.join(file_name).as_std_path())
        .assert()
        .success()
        .stdout("| A | B |\n| 1 | 2 |\n| 3 | 4 |\n");
}

/// Snapshots equivalent stdout and in-place output for a representative file.
#[test]
fn cli_output_modes_snapshot_table_prose() {
    let input = include_str!("data/cli-output-parity.md");
    let dir = tempdir().expect("failed to create temporary directory");
    let (directory, parent_path) = capability_directory(&dir);
    let stdout_path = parent_path.join("stdout.md");
    let in_place_path = parent_path.join("in-place.md");
    directory
        .write("stdout.md", input)
        .expect("failed to write stdout fixture");
    directory
        .write("in-place.md", input)
        .expect("failed to write in-place fixture");

    let stdout = Command::cargo_bin("mdtablefix")
        .expect("failed to create cargo command for mdtablefix")
        .arg(stdout_path.as_std_path())
        .output()
        .expect("failed to run stdout command");
    assert!(
        stdout.status.success(),
        "stdout command failed: {}",
        String::from_utf8_lossy(&stdout.stderr)
    );
    insta::assert_snapshot!(
        "format_to_string_table_prose",
        String::from_utf8_lossy(&stdout.stdout)
    );

    let in_place = Command::cargo_bin("mdtablefix")
        .expect("failed to create cargo command for mdtablefix")
        .args(["--in-place", in_place_path.as_str()])
        .output()
        .expect("failed to run in-place command");
    assert!(
        in_place.status.success(),
        "in-place command failed: {}",
        String::from_utf8_lossy(&in_place.stderr)
    );
    assert!(
        in_place.stdout.is_empty(),
        "in-place command wrote unexpected stdout: {}",
        String::from_utf8_lossy(&in_place.stdout)
    );
    insta::assert_snapshot!(
        "rewrite_in_place_table_prose",
        directory
            .read_to_string("in-place.md")
            .expect("failed to read rewritten fixture")
    );
}

/// Tests that the `--fences` option normalizes backtick fences.
#[test]
fn test_cli_fences_option() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--fences")
        .write_stdin("````rust\nfn main() {}\n````\n")
        .assert()
        .success()
        .stdout("```rust\nfn main() {}\n```\n");
}

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

#[test]
fn test_cli_fences_option_tilde() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--fences")
        .write_stdin("~~~~rust\nfn main() {}\n~~~~\n")
        .assert()
        .success()
        .stdout("```rust\nfn main() {}\n```\n");
}

/// Ensures fence normalization runs before other processing.
#[test]
fn test_cli_fences_before_ellipsis() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .args(["--fences", "--ellipsis"])
        .write_stdin("````\nlet x = ...;\n````\n")
        .assert()
        .success()
        .stdout("```\nlet x = ...;\n```\n");
}

/// Ensures orphan specifiers are attached when `--fences` is used.
#[test]
fn test_cli_fences_orphan_specifier() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--fences")
        .write_stdin("Rust\n```\nfn main() {}\n```\n")
        .assert()
        .success()
        .stdout("```rust\nfn main() {}\n```\n");
}

/// Combines fence normalization with renumbering to verify processing order.
#[test]
fn test_cli_fences_with_renumber() {
    let input = concat!(
        "Rust\n",
        "\n",
        "~~~~~~\n",
        "fn main() {}\n",
        "~~~~~~\n",
        "\n",
        "1. first\n",
        "3. second\n",
    );
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .args(["--fences", "--renumber"])
        .write_stdin(input)
        .assert()
        .success()
        .stdout("```rust\nfn main() {}\n```\n\n1. first\n2. second\n");
}

#[test]
fn test_cli_fences_preserve_existing_language() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--fences")
        .write_stdin("ruby\n```rust\nfn main() {}\n```\n")
        .assert()
        .success()
        .stdout("ruby\n```rust\nfn main() {}\n```\n");
}

#[test]
fn test_cli_fences_orphan_specifier_symbols() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--fences")
        .write_stdin("C++\n```\nfn main() {}\n```\n")
        .assert()
        .success()
        .stdout("```c++\nfn main() {}\n```\n");
}

#[test]
fn test_cli_no_attach_without_preceding_blank_line() {
    let input = concat!("text\n", "Rust\n", "```\n", "fn main() {}\n", "```\n");
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--fences")
        .write_stdin(input)
        .assert()
        .success()
        .stdout("text\nRust\n```\nfn main() {}\n```\n");
}

/// Tests the CLI `--footnotes` option to convert bare footnote links.
#[test]
fn test_cli_footnotes_option() {
    let input = include_str!("data/footnotes_input.txt");
    let expected = include_str!("data/footnotes_expected.txt");
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--footnotes")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(format!("{}\n", expected.trim_end()));
}

/// Executes an in-place rewrite with the provided flags and asserts idempotence.
fn run_in_place(flags: &[&str], input: &str, expected: &str) {
    let dir = tempdir().expect("failed to create temporary directory");
    let (directory, parent_path) = capability_directory(&dir);
    let file_name = Utf8Path::new("sample.md");
    let file_path = parent_path.join(file_name);
    directory
        .write(file_name, input)
        .expect("failed to write test file");

    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .args(["--in-place"])
        .args(flags)
        .arg(file_path.as_std_path())
        .assert()
        .success()
        .stdout("")
        .stderr("");

    let out = directory
        .read_to_string(file_name)
        .expect("failed to read output file");
    assert_eq!(out.trim_end(), expected.trim_end());
    assert!(
        out.ends_with('\n'),
        "output file must end with a trailing newline"
    );

    // idempotence
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .args(["--in-place"])
        .args(flags)
        .arg(file_path.as_std_path())
        .assert()
        .success()
        .stdout("")
        .stderr("");

    let out2 = directory
        .read_to_string(file_name)
        .expect("failed to read output file");
    assert!(
        out2.ends_with('\n'),
        "output file must end with a trailing newline"
    );
    assert_eq!(out2, out);
}

/// Opens a temporary directory as the capability-scoped I/O boundary for a test.
fn capability_directory(tempdir: &TempDir) -> (Dir, Utf8PathBuf) {
    let path = Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
        .expect("temporary directory path is UTF-8");
    let directory = Dir::open_ambient_dir(&path, ambient_authority())
        .expect("failed to open temporary directory");
    (directory, path)
}

/// Ensures `--in-place` rewrites files correctly for multiple flag combinations.
#[rstest]
#[case(&["--fences"], "Rust\n```\nfn main() {}\n```\n", "```rust\nfn main() {}\n```\n")]
#[case(&["--footnotes"], include_str!("data/footnotes_input.txt"), include_str!("data/footnotes_expected.txt"))]
#[case(&["--fences", "--footnotes"], include_str!("data/fences_footnotes_input.txt"), include_str!("data/fences_footnotes_expected.txt"))]
#[case(&["--wrap", "--footnotes"], include_str!("data/footnotes_input.txt"), include_str!("data/footnotes_wrap_expected.txt"))]
#[case(&["--wrap", "--ellipsis"], include_str!("data/ellipsis_wrap_input.txt"), include_str!("data/ellipsis_wrap_expected.txt"))]
#[case(&["--headings"], "Title\n=====\n", "# Title\n")]
fn test_cli_in_place_variants(#[case] flags: &[&str], #[case] input: &str, #[case] expected: &str) {
    run_in_place(flags, input, expected);
}

#[rstest]
#[case("```null\nfn main() {}\n```\n", "```\nfn main() {}\n```\n")]
#[case("```NULL\nfn main() {}\n```\n", "```\nfn main() {}\n```\n")]
#[case("```Null\nfn main() {}\n```\n", "```\nfn main() {}\n```\n")]
#[case("```null  \nfn main() {}\n```\n", "```\nfn main() {}\n```\n")]
#[case("```NULL  \nfn main() {}\n```\n", "```\nfn main() {}\n```\n")]
#[case("```Null  \nfn main() {}\n```\n", "```\nfn main() {}\n```\n")]
#[case("~~~~null\nfn main() {}\n~~~~\n", "```\nfn main() {}\n```\n")]
#[case("~~~~NULL\nfn main() {}\n~~~~\n", "```\nfn main() {}\n```\n")]
#[case("~~~~Null\nfn main() {}\n~~~~\n", "```\nfn main() {}\n```\n")]
#[case("~~~~null  \nfn main() {}\n~~~~\n", "```\nfn main() {}\n```\n")]
#[case("~~~~NULL  \nfn main() {}\n~~~~\n", "```\nfn main() {}\n```\n")]
#[case("~~~~Null  \nfn main() {}\n~~~~\n", "```\nfn main() {}\n```\n")]
#[case("  ```null\nfn main() {}\n```\n", "  ```\nfn main() {}\n```\n")]
fn test_cli_fences_null_language(#[case] input: &str, #[case] expected: &'static str) {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--fences")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(expected);
}
