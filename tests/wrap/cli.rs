//! CLI wrapping option tests.
//!
//! Validates that the `--wrap` command-line flag correctly triggers text
//! wrapping behaviour when processing Markdown content through the `mdtablefix`
//! binary.

use super::*;

#[test]
fn test_cli_wrap_option() {
    let input = "This line is deliberately made much longer than eighty columns so that the \
                 wrapping algorithm is forced to insert a soft line-break somewhere in the middle \
                 of the paragraph when the --wrap flag is supplied.";
    let assert = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--wrap")
        .write_stdin(format!("{input}\n"))
        .assert()
        .success();
    let text = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(text.lines().count() > 1, "expected wrapped output on multiple lines");
    assert!(text.lines().all(|l| l.len() <= 80));
}
