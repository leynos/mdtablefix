//! File-backed CLI regression tests for the parameterless wrapping flag.

use assert_cmd::Command;
use mdtablefix::process::WRAP_COLS;
use unicode_width::UnicodeWidthStr;

#[path = "../common/fs.rs"]
mod test_fs;
use test_fs::TestDir;

/// Ensures a path after `--wrap` remains a positional input file.
#[test]
fn cli_wrap_processes_positional_file() -> Result<(), Box<dyn std::error::Error>> {
    let input = concat!(
        "This file-backed paragraph is deliberately long enough to require wrapping when the ",
        "parameterless flag processes its positional input file rather than treating the path ",
        "as a wrap width.\n",
        "漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂 ",
        "漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂 漢字🙂.\n",
    );
    let dir = TestDir::new()?;
    dir.directory().write("input.md", input)?;
    let file_path = dir.path().join("input.md");

    let mut command = Command::cargo_bin("mdtablefix")?;
    let output = command
        .arg("--wrap")
        .arg(file_path.as_std_path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output)?;

    assert!(text.lines().count() > 1, "expected the file to be wrapped");
    assert!(
        text.contains("漢字🙂"),
        "expected positional file content to be preserved: {text}",
    );
    assert!(
        text.lines()
            .all(|line| UnicodeWidthStr::width(line) <= WRAP_COLS),
        "expected every output line to fit the fixed wrap width: {text}",
    );
    Ok(())
}
