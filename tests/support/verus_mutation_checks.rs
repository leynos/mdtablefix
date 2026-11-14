//! Mutation-gate attribution checks with the existing scoped fake runner.
//!
//! A negative control is only evidence if it fails for its stated reason, so
//! the contract gates are exercised in nine shapes: the intended rejection, the
//! same rejection followed by a trigger-suggestion note, a run that verifies, a
//! runner that never reaches the verifier, a failure attributed to an unrelated
//! module, a failure that is not a failed postcondition at all, a postcondition
//! failure with no source location, an intended rejection sitting beside a
//! coded compiler error, and one carrying two source locations. Only the first
//! two may pass, and the second is there because a note's own location is not a
//! second location of the error above it.

use anyhow::{Context, Result, ensure};
use rstest::rstest;

use super::{fake_prover_tools, make_command, runner_log};

/// One fake run's shape and the verdict its gate must give it.
///
/// The gates answer the same question and differ only in these six values, so
/// the expectation travels as one record. Passing them positionally made every
/// call site a place to transpose two adjacent strings, which would have
/// swapped a diagnostic for a confirmation without failing to compile.
#[derive(Clone, Copy)]
struct GateExpectation {
    /// The Makefile target under test.
    target: &'static str,
    /// The fake runner's mode, which selects the injected transcript.
    mode: &'static str,
    /// Whether this shape is a detection the gate should accept.
    should_succeed: bool,
    /// Text the gate's standard error must contain on a refusal.
    diagnostic: Option<&'static str>,
    /// Text the gate's standard output must contain on the intended rejection.
    confirmation: Option<&'static str>,
    /// The mirror directory the handoff check looks for in the runner's log.
    proof_dir_marker: &'static str,
}

/// The verdict a gate gave a fake run, checked against what it should give.
fn assert_gate_verdict(expectation: GateExpectation) -> Result<()> {
    let GateExpectation {
        target,
        mode,
        should_succeed,
        diagnostic: expected_diagnostic,
        confirmation: expected_confirmation,
        proof_dir_marker,
    } = expectation;
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
#[case::intended_rejection_then_note(
    "emission_trigger_note",
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
#[case::coded_compiler_error(
    "emission_coded_error",
    false,
    Some("Verifier failed with an error that was not a failed postcondition"),
    None
)]
#[case::two_source_locations(
    "emission_double_location",
    false,
    Some("Verifier reported an error with more than one source location"),
    None
)]
fn emission_mutation_gate_requires_an_attributed_rejection(
    #[case] mode: &'static str,
    #[case] should_succeed: bool,
    #[case] expected_diagnostic: Option<&'static str>,
    #[case] expected_confirmation: Option<&'static str>,
) -> Result<()> {
    assert_gate_verdict(GateExpectation {
        target: "verus-emission-mutation",
        mode,
        should_succeed,
        diagnostic: expected_diagnostic,
        confirmation: expected_confirmation,
        proof_dir_marker: ".emission-mutation-",
    })
}

/// The blank-tag gate rejects only a failure it can attribute.
#[rstest]
#[case::witness_postcondition(
    "blank_tag_rejected",
    true,
    None,
    Some("confirmed the expected read-back rejection")
)]
#[case::intended_rejection_then_note(
    "blank_tag_trigger_note",
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
#[case::coded_compiler_error(
    "blank_tag_coded_error",
    false,
    Some("Verifier failed with an error that was not a failed postcondition"),
    None
)]
#[case::two_source_locations(
    "blank_tag_double_location",
    false,
    Some("Verifier reported an error with more than one source location"),
    None
)]
fn blank_tag_mutation_gate_requires_an_attributed_rejection(
    #[case] mode: &'static str,
    #[case] should_succeed: bool,
    #[case] expected_diagnostic: Option<&'static str>,
    #[case] expected_confirmation: Option<&'static str>,
) -> Result<()> {
    assert_gate_verdict(GateExpectation {
        target: "verus-blank-tag-mutation",
        mode,
        should_succeed,
        diagnostic: expected_diagnostic,
        confirmation: expected_confirmation,
        proof_dir_marker: ".blank-tag-mutation-",
    })
}
