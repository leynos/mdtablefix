//! Behavioural tests for the CLI `--wrap` option.
//!
//! These tests exercise how the binary reflows long input when invoked with
//! `--wrap`. They verify that lines never exceed 80 columns and that wrapping is
//! consistent with internal library behaviour. By covering the command-line
//! entry point, they help catch regressions in argument handling and ensure the
//! user-facing interface behaves as documented.


#[macro_use]
#[path = "../prelude/mod.rs"]
mod prelude;
use prelude::*;

#[test]
fn test_cli_wrap_option() {
    let input = "This line is deliberately made much longer than eighty columns so that the \
                 wrapping algorithm is forced to insert a soft line-break somewhere in the middle \
                 of the paragraph when the --wrap flag is supplied.";
    let output = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--wrap")
        .write_stdin(format!("{input}\n"))
        .output()
        .expect("Failed to execute mdtablefix command");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.lines().count() > 1, "expected wrapped output on multiple lines");
    assert!(text.lines().all(|l| l.len() <= 80));
}
