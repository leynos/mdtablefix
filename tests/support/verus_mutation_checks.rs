//! Mutation-gate attribution checks with the existing scoped fake runner.
//!
//! A negative control is only evidence if it fails for its stated reason, so
//! the contract gates are exercised in six shapes: the intended rejection, a
//! run that verifies, a runner that never reaches the verifier, a failure
//! attributed to an unrelated module, a failure that is not a failed
//! postcondition at all, and a postcondition failure with no source location.
//! Only the first may pass.

use anyhow::{Context, Result, ensure};
use rstest::rstest;

use super::{fake_prover_tools, make_command, runner_log};

/// The verdict a gate gave a fake run, checked against what it should give.
///
/// The gates share this shape because they answer the same question, and they
/// differ only in the target, the injected mode, the diagnostic that names the
/// reason for a rejection, and the mirror directory their transcript names.
fn assert_gate_verdict(
    target: &str,
    mode: &'static str,
    should_succeed: bool,
    expected_diagnostic: Option<&str>,
    expected_confirmation: Option<&str>,
    proof_dir_marker: &str,
) -> Result<()> {
    let runner = fake_prover_tools(mode)?;
    let output = make_command(target, &runner)
        .output()
        .with_context(|| format!("run make {target} with fake prover-tools"))?;
    assert_eq!(
        output.status.success(),
        should_succeed,
        "{target} in mode {mode}: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    if let Some(expected) = expected_diagnostic {
        let stderr = String::from_utf8_lossy(&output.stderr);
        ensure!(
            stderr.contains(expected),
            "{target} in mode {mode} should report {expected:?}, got: {stderr}"
        );
    }
    if let Some(expected) = expected_confirmation {
        let stdout = String::from_utf8_lossy(&output.stdout);
        ensure!(
            stdout.contains(expected),
            "{target} in mode {mode} should confirm {expected:?}"
        );
    }
    ensure!(
        runner_log(&runner)?
            .lines()
            .any(|line| line.contains(proof_dir_marker)),
        "{target} should hand the verifier its mirrored proof tree"
    );
    Ok(())
}

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

/// The normalized-emission gate rejects only a failure it can attribute.
#[rstest]
#[case::consumer_and_witness(
    "emission_rejected",
    true,
    None,
    Some("confirmed the expected read-back rejection")
)]
#[case::unexpected_success(
    "emission_accepted",
    false,
    Some("Normalized-emission mutation unexpectedly verified"),
    None
)]
#[case::runner_failure(
    "emission_runner_failure",
    false,
    Some("Verifier reported no error at all"),
    None
)]
#[case::unrelated_module(
    "emission_unrelated_failure",
    false,
    Some("Verifier failed outside the expected modules: plan_spec_agreement.rs"),
    None
)]
#[case::not_a_postcondition(
    "emission_parse_error",
    false,
    Some("Verifier failed with an error that was not a failed postcondition"),
    None
)]
#[case::unlocated_postcondition(
    "emission_unlocated_failure",
    false,
    Some("Verifier reported an error with no source location"),
    None
)]
fn emission_mutation_gate_requires_an_attributed_rejection(
    #[case] mode: &'static str,
    #[case] should_succeed: bool,
    #[case] expected_diagnostic: Option<&str>,
    #[case] expected_confirmation: Option<&str>,
) -> Result<()> {
    assert_gate_verdict(
        "verus-emission-mutation",
        mode,
        should_succeed,
        expected_diagnostic,
        expected_confirmation,
        ".emission-mutation-",
    )
}

/// The blank-tag gate rejects only a failure it can attribute.
#[rstest]
#[case::witness_postcondition(
    "blank_tag_rejected",
    true,
    None,
    Some("confirmed the expected read-back rejection")
)]
#[case::unexpected_success(
    "blank_tag_accepted",
    false,
    Some("Blank-tag mutation unexpectedly verified"),
    None
)]
#[case::runner_failure(
    "blank_tag_runner_failure",
    false,
    Some("Verifier reported no error at all"),
    None
)]
#[case::unrelated_module(
    "blank_tag_unrelated_failure",
    false,
    Some("Verifier failed outside the expected modules: plan_spec_emission.rs"),
    None
)]
#[case::not_a_postcondition(
    "blank_tag_precondition_failure",
    false,
    Some("Verifier failed with an error that was not a failed postcondition"),
    None
)]
#[case::unlocated_postcondition(
    "blank_tag_unlocated_failure",
    false,
    Some("Verifier reported an error with no source location"),
    None
)]
fn blank_tag_mutation_gate_requires_an_attributed_rejection(
    #[case] mode: &'static str,
    #[case] should_succeed: bool,
    #[case] expected_diagnostic: Option<&str>,
    #[case] expected_confirmation: Option<&str>,
) -> Result<()> {
    assert_gate_verdict(
        "verus-blank-tag-mutation",
        mode,
        should_succeed,
        expected_diagnostic,
        expected_confirmation,
        ".blank-tag-mutation-",
    )
}
