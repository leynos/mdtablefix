//! CLI standard input helpers used by integration tests.

use assert_cmd::{Command, assert::Assert};

pub type CliResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Run the `mdtablefix` binary with the provided arguments and standard input.
///
/// Returns an [`Assert`] handle for chaining output and status checks.
pub fn run_cli_with_stdin(args: &[&str], input: &str) -> CliResult<Assert> {
    let mut command = Command::cargo_bin("mdtablefix")?;
    let assertion = command.args(args).write_stdin(input.to_owned()).assert();
    Ok(assertion)
}
