//! Integration tests for the `--code-emphasis` flag.
//!
//! Verifies that emphasis markers adjacent to inline code are normalized.

use rstest::rstest;

#[path = "support/cli_args.rs"]
mod cli_args;
#[path = "support/cli_stdin.rs"]
mod cli_stdin;
#[path = "common/fs.rs"]
mod test_fs;
use cli_args::run_cli_with_args;
use cli_stdin::run_cli_with_stdin;
use test_fs::TestDir;

fn assert_in_place_result(file_name: &str, input: &str, expected: &str) {
    let dir = TestDir::new().expect("failed to create temporary directory");
    dir.directory()
        .write(file_name, input)
        .expect("failed to write test file");
    let file_path = dir.path().join(file_name);

    run_cli_with_args(&["--code-emphasis", "--in-place", file_path.as_str()])
        .expect("failed to run mdtablefix")
        .success()
        .stdout("");

    let output = dir
        .directory()
        .read_to_string(file_name)
        .expect("failed to read output file");
    assert_eq!(output, expected);
}

#[test]
fn cli_stdin_code_emphasis() -> Result<(), Box<dyn std::error::Error>> {
    let input = "`StepContext`** Enhancement (in **`crates/rstest-bdd/src/context.rs`**)**\n";
    let expected = "**`StepContext` Enhancement (in `crates/rstest-bdd/src/context.rs`)**\n";
    let assertion = run_cli_with_stdin(&["--code-emphasis"], input)?;
    assertion.success().stdout(expected);
    Ok(())
}

#[test]
fn cli_without_flag_is_noop_for_code_emphasis_input() -> Result<(), Box<dyn std::error::Error>> {
    let input = "`StepContext`** Enhancement (in **`crates/rstest-bdd/src/context.rs`**)**\n";
    let assertion = run_cli_with_stdin(&[], input)?;
    assertion.success().stdout(input);
    Ok(())
}

#[rstest]
#[case("*`VarGuard`s*\n")]
#[case("**`code`**\n")]
fn cli_preserves_emphasised_code(
    #[case] input: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    let assertion = run_cli_with_stdin(&["--code-emphasis"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

#[test]
fn cli_in_place_code_emphasis() {
    let input = "`StepContext`** Enhancement (in **`crates/rstest-bdd/src/context.rs`**)**\n";
    let expected = "**`StepContext` Enhancement (in `crates/rstest-bdd/src/context.rs`)**\n";
    assert_in_place_result("sample.md", input, expected);
}

#[test]
fn cli_in_place_code_emphasis_empty_file() { assert_in_place_result("empty.md", "", ""); }

#[test]
fn cli_in_place_code_emphasis_whitespace_file() {
    let input = "   \n\t  ";
    let expected = "   \n\t  \n";
    assert_in_place_result("whitespace.md", input, expected);
}

#[test]
fn cli_in_place_preserves_inner_backticks() {
    let input = "```` ``a`b`` ````\n";
    assert_in_place_result("inner.md", input, input);
}

#[test]
fn cli_code_emphasis_with_wrap_and_renumber() -> Result<(), Box<dyn std::error::Error>> {
    let input = "8. `StepContext`** Enhancement (in \
                 **`crates/rstest-bdd/src/context.rs`**)**\n10. Second item\n";
    let expected = "1. **`StepContext` Enhancement (in `crates/rstest-bdd/src/context.rs`)**\n2. \
                    Second item\n";
    let assertion = run_cli_with_stdin(&["--code-emphasis", "--wrap", "--renumber"], input)?;
    assertion.success().stdout(expected);
    Ok(())
}

#[test]
fn cli_preserves_inner_backticks() -> Result<(), Box<dyn std::error::Error>> {
    let input = "``a`b``\n";
    let assertion = run_cli_with_stdin(&["--code-emphasis"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

#[test]
fn cli_preserves_standalone_code() -> Result<(), Box<dyn std::error::Error>> {
    let input = "`code` text\n";
    let assertion = run_cli_with_stdin(&["--code-emphasis"], input)?;
    assertion.success().stdout(input);
    Ok(())
}
