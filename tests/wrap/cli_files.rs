//! File-backed CLI regression tests for the parameterless wrapping flag.

use std::fs;

use assert_cmd::Command;
use tempfile::NamedTempFile;

/// Ensures a path after `--wrap` remains a positional input file.
#[test]
fn cli_wrap_processes_positional_file() -> Result<(), Box<dyn std::error::Error>> {
    let input = concat!(
        "This file-backed paragraph is deliberately long enough to require wrapping when the ",
        "parameterless flag processes its positional input file rather than treating the path ",
        "as a wrap width.\n",
    );
    let file = NamedTempFile::new()?;
    fs::write(file.path(), input)?;

    let mut command = Command::cargo_bin("mdtablefix")?;
    let output = command
        .arg("--wrap")
        .arg(file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output)?;

    assert!(text.lines().count() > 1, "expected the file to be wrapped");
    assert!(
        text.lines().all(|line| line.len() <= 80),
        "expected every output line to fit the fixed wrap width: {text}",
    );
    Ok(())
}
