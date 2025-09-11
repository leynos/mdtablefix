//! Integration tests for CLI interface behaviour of the `mdtablefix` tool.
//!
//! This module validates the command-line interface functionality, including:
//! - File handling with the `--in-place` flag
//! - Ellipsis replacement with the `--ellipsis` option
//! - Error handling for invalid argument combinations
//! - Processing of Markdown files through the CLI interface

use std::{
    fs::{self, File},
    io::Write,
};

use rstest::rstest;
use tempfile::tempdir;

#[macro_use]
mod prelude;
use prelude::*;

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
    let file_path = dir.path().join("sample.md");
    let mut f = File::create(&file_path).expect("failed to create temporary file");
    for line in &broken_table {
        writeln!(f, "{line}").expect("failed to write line");
    }
    f.flush().expect("failed to flush file");
    drop(f);
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg(&file_path)
        .assert()
        .success()
        .stdout("| A | B |\n| 1 | 2 |\n| 3 | 4 |\n");
}

/// Tests that the `--ellipsis` option replaces triple dots with a Unicode ellipsis character.
///
/// Verifies that the CLI correctly processes input containing "..." and outputs "…".
#[test]
fn test_cli_ellipsis_option() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("foo...\n")
        .assert()
        .success()
        .stdout("foo…\n");
}

/// Tests that the `--ellipsis` option preserves dots within inline code spans.
///
/// Verifies that triple dots inside backtick-delimited code spans are not converted to ellipsis.
#[test]
fn test_cli_ellipsis_code_span() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("before `dots...` after\n")
        .assert()
        .success()
        .stdout("before `dots...` after\n");
}

/// Tests that the `--ellipsis` option does not alter fenced code blocks.
///
/// Ensures that sequences like "..." inside a fenced code block remain unchanged.
#[test]
fn test_cli_ellipsis_fenced_block() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("```\nlet x = ...;\n```\n")
        .assert()
        .success()
        .stdout("```\nlet x = ...;\n```\n");
}

/// Tests ellipsis replacement for sequences longer than three characters.
///
/// Confirms that only the first three dots are replaced with an ellipsis.
#[test]
fn test_cli_ellipsis_long_sequence() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("wait....\n")
        .assert()
        .success()
        .stdout("wait….\n");
}

/// Tests that the `--ellipsis` option handles multiple ellipsis sequences in one line.
///
/// Verifies that all occurrences of "..." are replaced with "…".
#[test]
fn test_cli_ellipsis_multiple_sequences() {
    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--ellipsis")
        .write_stdin("First... then second... done.\n")
        .assert()
        .success()
        .stdout("First… then second… done.\n");
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
    let file_path = dir.path().join("sample.md");
    fs::write(&file_path, input).expect("failed to write test file");

    Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .args(["--in-place"])
        .args(flags)
        .arg(&file_path)
        .assert()
        .success()
        .stdout("")
        .stderr("");

    let out = fs::read_to_string(&file_path).expect("failed to read output file");
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
        .arg(&file_path)
        .assert()
        .success()
        .stdout("")
        .stderr("");

    let out2 = fs::read_to_string(&file_path).expect("failed to read output file");
    assert!(
        out2.ends_with('\n'),
        "output file must end with a trailing newline"
    );
    assert_eq!(out2, out);
}

/// Ensures `--in-place` rewrites files correctly for multiple flag combinations.
#[rstest]
#[case(&["--fences"], "Rust\n```\nfn main() {}\n```\n", "```rust\nfn main() {}\n```\n")]
#[case(&["--footnotes"], include_str!("data/footnotes_input.txt"), include_str!("data/footnotes_expected.txt"))]
#[case(&["--fences", "--footnotes"], include_str!("data/fences_footnotes_input.txt"), include_str!("data/fences_footnotes_expected.txt"))]
#[case(&["--wrap", "--footnotes"], include_str!("data/footnotes_input.txt"), include_str!("data/footnotes_wrap_expected.txt"))]
#[case(&["--wrap", "--ellipsis"], include_str!("data/ellipsis_wrap_input.txt"), include_str!("data/ellipsis_wrap_expected.txt"))]
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
