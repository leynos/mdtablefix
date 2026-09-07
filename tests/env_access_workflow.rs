//! Contract coverage for the continuous-integration side of the policy.
//!
//! `tests/env_access_policy.rs` proves the Makefile's `lint` target enforces
//! the environment-access policy, and `tests/env_access_enforcement.rs` proves
//! the lint fires. Neither says anything about whether CI runs any of it. A
//! workflow can keep a perfectly good command and never execute it: `if: false`
//! on the step, `if: false` on the job, a condition that is simply never true
//! on a pull request, or a removed `pull_request` trigger all leave the `run`
//! value untouched while the gate stops running. A green tick would then mean
//! nothing.
//!
//! So this file asserts the path from the trigger to the command: the
//! `pull_request` trigger exists, the job exists and carries no condition, and
//! the steps that run the lint target and the test suite carry none either.
//!
//! Two details matter. The lint step's *whole* `run` value must be the command,
//! not merely contain it, because `if false; then make lint; fi` contains it
//! while running nothing. And conditions are judged by presence rather than by
//! value: enumerating the spellings of a false condition is the same losing
//! game as enumerating the spellings of a disabled command, and a condition
//! that looks true can still be false on a pull request. Any condition on these
//! steps is a decision that deserves a fresh look at this contract.
//!
//! Mutation proof (2026-09-07). Each mutation was applied to
//! `.github/workflows/ci.yml` alone, the suite run, and the mutation reverted:
//!
//! ```text
//! if: false on the Lint step
//!   -> the Lint step must carry no condition, found Some(Bool(false))
//! if: false on the build-test job
//!   -> the build-test job must carry no condition, found Some(Bool(false))
//! if: false on the Test and Measure Coverage step
//!   -> the Test and Measure Coverage step must carry no condition,
//!      found Some(Bool(false))
//! if: ${{ github.event_name == 'push' }} on the Lint step
//!   -> the Lint step must carry no condition, found Some(String(...))
//! wrap the run value in `if false; then make lint; fi`
//!   -> no step runs `make lint` as its whole command
//! change the run value to `make typecheck`
//!   -> no step runs `make lint` as its whole command
//! delete the pull_request trigger
//!   -> the workflow must run on pull_request, found ["workflow_dispatch"]
//! ```

use anyhow::{Context, Result, ensure};
use serde_yaml::{Mapping, Value};

const WORKFLOW: &str = include_str!("../.github/workflows/ci.yml");

/// The job whose steps carry the policy into CI.
const GATE_JOB: &str = "build-test";

/// The step that runs the lint target, and the command it must run.
const LINT_STEP: (&str, &str) = ("Lint", "make lint");

/// The action that runs the test suite, which includes the policy contracts.
///
/// Matched on the part before the `@`, so a pin bump does not fail the
/// contract while a different action does.
const COVERAGE_ACTION: &str = "leynos/shared-actions/.github/actions/generate-coverage";

/// The step that runs the test suite.
const COVERAGE_STEP: &str = "Test and Measure Coverage";

/// Parse the workflow into its top-level mapping.
fn workflow() -> Result<Mapping> { serde_yaml::from_str(WORKFLOW).context("parse the CI workflow") }

/// Return the workflow's trigger mapping.
///
/// YAML 1.1 reads a bare `on` key as the boolean `true`, so the key is looked
/// up both ways rather than relying on the file continuing to quote it.
fn triggers(workflow: &Mapping) -> Result<&Mapping> {
    workflow
        .get(Value::from("on"))
        .or_else(|| workflow.get(Value::Bool(true)))
        .and_then(Value::as_mapping)
        .context("the workflow should declare its triggers")
}

/// Return a named job's mapping.
fn job<'a>(workflow: &'a Mapping, name: &str) -> Result<&'a Mapping> {
    workflow
        .get(Value::from("jobs"))
        .and_then(Value::as_mapping)
        .and_then(|jobs| jobs.get(Value::from(name)))
        .and_then(Value::as_mapping)
        .with_context(|| format!("the workflow should declare a {name} job"))
}

/// Return a job's steps.
fn steps<'a>(job: &'a Mapping, name: &str) -> Result<&'a Vec<Value>> {
    job.get(Value::from("steps"))
        .and_then(Value::as_sequence)
        .with_context(|| format!("the {name} job should declare steps"))
}

/// Return the step with the given `name`.
fn step<'a>(steps: &'a [Value], name: &str) -> Result<&'a Mapping> {
    steps
        .iter()
        .filter_map(Value::as_mapping)
        .find(|step| step.get(Value::from("name")).and_then(Value::as_str) == Some(name))
        .with_context(|| format!("the {GATE_JOB} job should have a {name} step"))
}

/// Fail if `entry` carries an `if` condition.
///
/// Presence is what is judged, not truth. A condition's value can be a template
/// that is false only on a pull request, and enumerating the ways to write one
/// is the same losing game as enumerating the ways to disable a command.
fn ensure_unconditional(entry: &Mapping, description: &str) -> Result<()> {
    let condition = entry.get(Value::from("if"));
    ensure!(
        condition.is_none(),
        "the {description} must carry no condition, found {condition:?}"
    );
    Ok(())
}

/// Scenario: the workflow's triggers are read.
/// Invariant: `pull_request` is among them, so the gate runs on the changes it
/// exists to judge. Without it every check is skipped and the pull request is
/// green because nothing looked.
#[test]
fn the_workflow_runs_on_pull_request() -> Result<()> {
    let workflow = workflow()?;
    let triggers = triggers(&workflow)?;
    let names: Vec<&str> = triggers.keys().filter_map(Value::as_str).collect();
    ensure!(
        triggers.contains_key(Value::from("pull_request")),
        "the workflow must run on pull_request, found {names:?}"
    );
    Ok(())
}

/// Scenario: the job carrying the policy's gates is read.
/// Invariant: it exists and carries no condition, so it cannot be skipped
/// wholesale while its steps still look correct.
#[test]
fn the_gate_job_runs_unconditionally() -> Result<()> {
    let workflow = workflow()?;
    let job = job(&workflow, GATE_JOB)?;
    ensure_unconditional(job, &format!("{GATE_JOB} job"))
}

/// Scenario: the step that runs the lint target is read.
/// Invariant: its whole `run` value is the command, and it carries no
/// condition. Matching a substring would accept `if false; then make lint; fi`,
/// which runs nothing while reading correctly.
#[test]
fn a_step_runs_the_lint_target_unconditionally() -> Result<()> {
    let (name, command) = LINT_STEP;
    let workflow = workflow()?;
    let job = job(&workflow, GATE_JOB)?;
    let steps = steps(job, GATE_JOB)?;
    let runs_command = steps
        .iter()
        .filter_map(Value::as_mapping)
        .filter_map(|step| step.get(Value::from("run")))
        .filter_map(Value::as_str)
        .any(|run| run.trim() == command);
    ensure!(
        runs_command,
        "no step runs `{command}` as its whole command"
    );
    ensure_unconditional(step(steps, name)?, &format!("{name} step"))
}

/// Scenario: the step that runs the test suite is read.
/// Invariant: it uses the coverage action and carries no condition. The policy
/// contracts are tests, so a skipped test step is a skipped contract, and every
/// other assertion in this file would still pass.
#[test]
fn a_step_runs_the_test_suite_unconditionally() -> Result<()> {
    let workflow = workflow()?;
    let job = job(&workflow, GATE_JOB)?;
    let steps = steps(job, GATE_JOB)?;
    let runs_tests = steps
        .iter()
        .filter_map(Value::as_mapping)
        .filter_map(|step| step.get(Value::from("uses")))
        .filter_map(Value::as_str)
        .any(|uses| uses.split('@').next() == Some(COVERAGE_ACTION));
    ensure!(runs_tests, "no step uses {COVERAGE_ACTION}");
    ensure_unconditional(
        step(steps, COVERAGE_STEP)?,
        &format!("{COVERAGE_STEP} step"),
    )
}
