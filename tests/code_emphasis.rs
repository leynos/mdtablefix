//! Integration tests for the `--code-emphasis` flag.
//!
//! Verifies that emphasis markers adjacent to inline code are normalised.

#[macro_use]
mod prelude;
use prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn cli_stdin_code_emphasis() {
    let input = "`StepContext`** Enhancement (in **`crates/rstest-bdd/src/context.rs`**)**\n";
    let expected = "**`StepContext` Enhancement (in `crates/rstest-bdd/src/context.rs`)**\n";
    run_cli_with_stdin(&["--code-emphasis"], input)
        .success()
        .stdout(expected);
}

#[test]
fn cli_preserves_emphasised_code_only() {
    let input = "**`code`**\n";
    run_cli_with_stdin(&["--code-emphasis"], input)
        .success()
        .stdout(input);
}

#[test]
fn cli_in_place_code_emphasis() {
    let dir = tempdir().expect("failed to create temporary directory");
    let file_path = dir.path().join("sample.md");
    let input = "`StepContext`** Enhancement (in **`crates/rstest-bdd/src/context.rs`**)**\n";
    let expected = "**`StepContext` Enhancement (in `crates/rstest-bdd/src/context.rs`)**\n";
    fs::write(&file_path, input).expect("failed to write test file");
    run_cli_with_args(&[
        "--code-emphasis",
        "--in-place",
        file_path.to_str().expect("path is not valid UTF-8"),
    ])
    .success()
    .stdout("");
    let out = fs::read_to_string(&file_path).expect("failed to read output file");
    assert_eq!(out, expected);
}

#[test]
fn cli_in_place_code_emphasis_empty_file() {
    let dir = tempdir().expect("failed to create temporary directory");
    let file_path = dir.path().join("empty.md");
    fs::write(&file_path, "").expect("failed to write test file");
    run_cli_with_args(&[
        "--code-emphasis",
        "--in-place",
        file_path.to_str().expect("path is not valid UTF-8"),
    ])
    .success()
    .stdout("");
    let out = fs::read_to_string(&file_path).expect("failed to read output file");
    assert_eq!(out, "");
}

#[test]
fn cli_in_place_code_emphasis_whitespace_file() {
    let dir = tempdir().expect("failed to create temporary directory");
    let file_path = dir.path().join("whitespace.md");
    let input = "   \n\t  ";
    let expected = "   \n\t  \n";
    fs::write(&file_path, input).expect("failed to write test file");
    run_cli_with_args(&[
        "--code-emphasis",
        "--in-place",
        file_path.to_str().expect("path is not valid UTF-8"),
    ])
    .success()
    .stdout("");
    let out = fs::read_to_string(&file_path).expect("failed to read output file");
    assert_eq!(out, expected);
}

#[test]
fn cli_code_emphasis_with_wrap_and_renumber() {
    let input = "8. `StepContext`** Enhancement (in **`crates/rstest-bdd/src/context.rs`**)**\n10. Second item\n";
    let expected = "1. **`StepContext` Enhancement (in `crates/rstest-bdd/src/context.rs`)**\n2. Second item\n";
    run_cli_with_stdin(&["--code-emphasis", "--wrap", "--renumber"], input)
        .success()
        .stdout(expected);
}

#[test]
fn cli_preserves_inner_backticks() {
    let input = "``a`b``\n";
    run_cli_with_stdin(&["--code-emphasis"], input)
        .success()
        .stdout(input);
}
