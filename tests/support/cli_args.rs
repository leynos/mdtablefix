//! CLI argument helpers used by integration tests.

use assert_cmd::{Command, assert::Assert};

/// Run the `mdtablefix` binary with the provided arguments.
///
/// Returns an [`Assert`] handle for chaining output and status checks.
pub fn run_cli_with_args(args: &[&str]) -> Result<Assert, Box<dyn std::error::Error>> {
    Ok(Command::cargo_bin("mdtablefix")?.args(args).assert())
}
