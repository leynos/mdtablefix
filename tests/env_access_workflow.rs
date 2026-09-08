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
//! `pull_request` trigger exists, and the job and the steps that run the lint
//! target and the test suite are each reachable and blocking.
//!
//! Two keys make a step non-blocking and they are not interchangeable. `if`
//! decides whether it runs at all; `continue-on-error` lets it run, fail, and
//! report success anyway. A contract that rejected only the first would accept
//! a gate that runs, finds the policy broken, and goes green.
//!
//! Three details matter. The lint step's *whole* `run` value must be the
//! command, not merely contain it, because `if false; then make lint; fi`
//! contains it while running nothing. The command and the condition are checked
//! on the *same* step: looked up separately, an unconditional step named `Lint`
//! that runs something else would vouch for a second step carrying the real
//! command behind `if: false`. And conditions are judged by presence rather
//! than by value: enumerating the spellings of a false condition is the same
//! losing game as enumerating the spellings of a disabled command, and a
//! condition that looks true can still be false on a pull request. Any
//! condition on these steps is a decision that deserves a fresh look at this
//! contract.
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
//! keep an unconditional step named Lint running something else, and put
//! `make lint` in a second step behind `if: false`
//!   -> every step that runs `make lint` as its whole command carries a
//!      condition, found [Bool(false)]
//! do the same for the coverage action
//!   -> every step that uses [the coverage action] carries a condition,
//!      found [Bool(false)]
//! continue-on-error: true on the Lint step
//!   -> every step that runs `make lint` as its whole command is non-blocking:
//!      sets continue-on-error to Bool(true), so its failure is ignored
//! continue-on-error: true on the build-test job
//!   -> the build-test job sets continue-on-error to Bool(true), so its
//!      failure is ignored
//! ```
//!
//! Adding a second, skipped step running `make lint` alongside the real one was
//! also applied, and passes: the gate still runs, so there is nothing to fail.

use anyhow::{Context, Result, bail, ensure};
use serde_yaml::{Mapping, Value};

const WORKFLOW: &str = include_str!("../.github/workflows/ci.yml");

/// The job whose steps carry the policy into CI.
const GATE_JOB: &str = "build-test";

/// The command the lint step must run.
const LINT_COMMAND: &str = "make lint";

/// The action that runs the test suite, which includes the policy contracts.
///
/// Matched on the part before the `@`, so a pin bump does not fail the
/// contract while a different action does.
const COVERAGE_ACTION: &str = "leynos/shared-actions/.github/actions/generate-coverage";

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

/// Fail if `entry` carries an `if` condition.
///
/// Presence is what is judged, not truth. A condition's value can be a template
/// that is false only on a pull request, and enumerating the ways to write one
/// is the same losing game as enumerating the ways to disable a command.
fn ensure_unconditional(entry: &Mapping, description: &str) -> Result<()> {
    if let Some(reason) = non_blocking(entry) {
        bail!("the {description} {reason}");
    }
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

/// Return why `entry` would not block a merge on failure, if it would not.
///
/// Two keys do it, and they are not interchangeable. `if` decides whether the
/// step or job runs at all; `continue-on-error` lets it run, fail, and report
/// success anyway. A contract that rejected only the first would accept a gate
/// that runs, finds the policy broken, and goes green.
fn non_blocking(entry: &Mapping) -> Option<String> {
    if let Some(condition) = entry.get(Value::from("if")) {
        return Some(format!("carries the condition {condition:?}"));
    }
    entry
        .get(Value::from("continue-on-error"))
        .map(|value| format!("sets continue-on-error to {value:?}, so its failure is ignored"))
}

/// Fail unless some step matching `selects` carries no condition.
///
/// The match and the condition check are deliberately the same step. Looking
/// the command up across every step and then the condition up on a step chosen
/// by name lets a workflow keep an unconditional step with the expected name
/// while a second step carries the real command behind `if: false`, at which
/// point both halves pass and the gate never runs.
///
/// One unconditional match is enough. A second, skipped step running the same
/// command changes nothing: the gate still runs.
fn ensure_some_step_runs_unconditionally(
    steps: &[Value],
    description: &str,
    selects: impl Fn(&Mapping) -> bool,
) -> Result<()> {
    let matching: Vec<&Mapping> = steps
        .iter()
        .filter_map(Value::as_mapping)
        .filter(|step| selects(step))
        .collect();
    ensure!(!matching.is_empty(), "no step {description}");
    let excuses: Vec<String> = matching
        .iter()
        .filter_map(|step| non_blocking(step))
        .collect();
    ensure!(
        matching.len() > excuses.len(),
        "every step that {description} is non-blocking: {}",
        excuses.join("; ")
    );
    Ok(())
}

/// Return whether `step`'s whole `run` value is `command`.
fn runs_command(step: &Mapping, command: &str) -> bool {
    step.get(Value::from("run"))
        .and_then(Value::as_str)
        .is_some_and(|run| run.trim() == command)
}

/// Return whether `step` uses `action`, whatever it is pinned to.
fn uses_action(step: &Mapping, action: &str) -> bool {
    step.get(Value::from("uses"))
        .and_then(Value::as_str)
        .is_some_and(|uses| uses.split('@').next() == Some(action))
}

/// Scenario: the steps are searched for one that runs the lint target.
/// Invariant: at least one step both runs the command as its whole `run` value
/// and carries no condition. Binding the two to the same step is the point:
/// checked separately, an unconditional step named `Lint` that runs something
/// else would vouch for a second step carrying `make lint` behind `if: false`.
/// Matching a substring rather than the whole value would accept
/// `if false; then make lint; fi`, which runs nothing while reading correctly.
#[test]
fn a_step_runs_the_lint_target_unconditionally() -> Result<()> {
    let workflow = workflow()?;
    let job = job(&workflow, GATE_JOB)?;
    let steps = steps(job, GATE_JOB)?;
    ensure_some_step_runs_unconditionally(
        steps,
        &format!("runs `{LINT_COMMAND}` as its whole command"),
        |step| runs_command(step, LINT_COMMAND),
    )
}

/// Scenario: the steps are searched for one that runs the test suite.
/// Invariant: at least one step both uses the coverage action and carries no
/// condition, bound to the same step for the same reason. The policy contracts
/// are tests, so a skipped test step is a skipped contract, and every other
/// assertion in this file would still pass.
#[test]
fn a_step_runs_the_test_suite_unconditionally() -> Result<()> {
    let workflow = workflow()?;
    let job = job(&workflow, GATE_JOB)?;
    let steps = steps(job, GATE_JOB)?;
    ensure_some_step_runs_unconditionally(steps, &format!("uses {COVERAGE_ACTION}"), |step| {
        uses_action(step, COVERAGE_ACTION)
    })
}
