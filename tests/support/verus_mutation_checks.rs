//! Mutation-gate attribution checks with the existing scoped fake runner.

use anyhow::{Context, Result, ensure};
use rstest::rstest;

use super::{fake_prover_tools, make_command, runner_log};

/// Only a completed output-classification failure is a successful mutation control.
#[rstest]
#[case::class_postcondition("mutation_class_rejected", true)]
#[case::unexpected_success("mutation_accepted", false)]
#[case::runner_failure("mutation_runner_failure", false)]
#[case::equality_only("mutation_equality_only", false)]
#[case::invariant_only("mutation_invariant_only", false)]
fn mutation_gate_requires_output_classification_failure(
    #[case] mode: &'static str,
    #[case] should_succeed: bool,
) -> Result<()> {
    let runner = fake_prover_tools(mode)?;
    let output = make_command("verus-mutation", &runner)
        .output()
        .context("run make verus-mutation with fake prover-tools")?;
    assert_eq!(
        output.status.success(),
        should_succeed,
        "mutation mode {mode}: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    if !should_succeed {
        let stderr = String::from_utf8_lossy(&output.stderr);
        ensure!(
            stderr.contains("ATX separator mutation unexpectedly verified")
                || stderr
                    .contains("did not fail the executable output-classification postcondition")
        );
    }
    ensure!(
        runner_log(&runner)?
            .lines()
            .any(|line| line.contains(".atx-mutation-"))
    );
    Ok(())
}
