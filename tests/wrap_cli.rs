//! CLI regression tests for wrap behaviour around verbatim code blocks.

use std::fs;

use assert_cmd::Command;
use rstest::rstest;
use tempfile::NamedTempFile;

fn run_wrap_in_place_and_read_back(
    input: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let temp = NamedTempFile::new()?;
    fs::write(temp.path(), input)?;

    let mut command = Command::cargo_bin("mdtablefix")?;
    command
        .args(["--wrap", "--in-place"])
        .arg(temp.path())
        .assert()
        .success()
        .stdout("")
        .stderr("");

    Ok(fs::read_to_string(temp.path())?)
}

/// Guards issue #261 by asserting `--wrap --in-place` leaves shell code blocks
/// byte-identical for both fenced and indented forms.
#[rstest]
#[case(
    concat!(
        "## Verification\n",
        "\n",
        "```bash\n",
        "set -o pipefail\n",
        "make check-fmt 2>&1 | tee /tmp/fmt.log\n",
        "make lint 2>&1 | tee /tmp/lint.log\n",
        "make test 2>&1 | tee /tmp/test.log\n",
        "```\n",
    ),
    "fenced code blocks must remain byte-identical",
)]
#[case(
    concat!(
        "## Verification\n",
        "\n",
        "    set -o pipefail\n",
        "    make check-fmt 2>&1 | tee /tmp/fmt.log\n",
        "    make lint 2>&1 | tee /tmp/lint.log\n",
        "    make test 2>&1 | tee /tmp/test.log\n",
    ),
    "indented code blocks must remain byte-identical",
)]
fn cli_wrap_in_place_preserves_shell_block_verbatim(
    #[case] input: &str,
    #[case] message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = run_wrap_in_place_and_read_back(input)?;
    assert_eq!(actual, input, "{message}");
    Ok(())
}

/// Guards issue #261 by asserting `--wrap --in-place` keeps fenced code blocks
/// intact even when there is no blank line after the heading, content follows
/// the block, and the source file lacks a trailing newline.
#[test]
fn cli_wrap_in_place_preserves_fenced_block_without_final_newline(
) -> Result<(), Box<dyn std::error::Error>> {
    let input = concat!(
        "## Verification\n",
        "```bash\n",
        "set -o pipefail\n",
        "make check-fmt 2>&1 | tee /tmp/fmt.log\n",
        "make lint 2>&1 | tee /tmp/lint.log\n",
        "make test 2>&1 | tee /tmp/test.log\n",
        "```\n",
        "Trailing paragraph without final newline",
    );
    let expected = concat!(
        "## Verification\n",
        "```bash\n",
        "set -o pipefail\n",
        "make check-fmt 2>&1 | tee /tmp/fmt.log\n",
        "make lint 2>&1 | tee /tmp/lint.log\n",
        "make test 2>&1 | tee /tmp/test.log\n",
        "```\n",
        "Trailing paragraph without final newline\n",
    );

    let actual = run_wrap_in_place_and_read_back(input)?;
    assert_eq!(actual, expected);
    Ok(())
}
