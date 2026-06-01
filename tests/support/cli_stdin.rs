//! CLI standard input helpers used by integration tests.

use assert_cmd::{Command, assert::Assert};

/// Run the `mdtablefix` binary with the provided arguments and standard input.
///
/// Returns an [`Assert`] handle for chaining output and status checks.
pub fn run_cli_with_stdin(
    args: &[&str],
    input: &str,
) -> Result<Assert, Box<dyn std::error::Error>> {
    Ok(Command::cargo_bin("mdtablefix")?
        .args(args)
        .write_stdin(input)
        .assert())
}
