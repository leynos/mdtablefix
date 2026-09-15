//! Fixtures for the two workflow spellings the contract reads rather than the
//! repository's own workflow.
//!
//! The assertions in the parent file are about `.github/workflows/ci.yml`, and
//! that file is written one way. A rule has to be proved in both directions,
//! and a rule the repository exercises in only one direction is a rule nobody
//! has tested: these cases supply the spellings the workflow does not use.

use anyhow::{Result, ensure};
use rstest::rstest;
use serde_yaml::{Mapping, Value};

use super::{is_false, non_blocking, trigger_events};

/// Parse a YAML fragment into a value the contract's helpers accept.
fn parse(source: &str) -> Result<Value> { Ok(serde_yaml::from_str(source)?) }

/// Parse a YAML mapping, for the entries `non_blocking` judges.
fn parse_mapping(source: &str) -> Result<Mapping> { Ok(serde_yaml::from_str(source)?) }

/// Scenario: the three forms GitHub accepts for a workflow's `on` key.
///
/// Invariant: each names `pull_request`, so the workflow runs on the changes
/// the gate exists to judge. Requiring the mapping form rejected the scalar and
/// the sequence, which run on a pull request exactly as the mapping does.
///
/// Mutation proof (2026-09-15), applied alone and reverted: restoring the
/// `Value::as_mapping` requirement, so only a mapping yields event names, fails
/// the scalar and sequence cases with `pull_request not among []`.
#[rstest]
#[case::scalar("pull_request")]
#[case::sequence("[pull_request, workflow_dispatch]")]
#[case::mapping("pull_request:\n  branches: [main]\npush:\n  branches: [main]")]
#[case::mapping_without_configuration("pull_request:\nworkflow_dispatch:")]
fn every_accepted_trigger_form_names_pull_request(#[case] source: &str) -> Result<()> {
    let triggers = parse(source)?;
    let events = trigger_events(&triggers);
    ensure!(
        events.contains(&"pull_request"),
        "pull_request not among {events:?}"
    );
    Ok(())
}

/// Scenario: trigger declarations that do not reach a pull request.
///
/// Invariant: none of them names `pull_request`. Without this the rule above
/// could be satisfied by a helper that reported the event whatever it read,
/// which would pass every mutation and guard nothing.
///
/// Mutation proof (2026-09-15), applied alone and reverted: returning
/// `vec!["pull_request"]` for every declaration fails all three cases with
/// `pull_request among ["pull_request"]`.
#[rstest]
#[case::scalar("push")]
#[case::sequence("[push, workflow_dispatch]")]
#[case::mapping("push:\n  branches: [main]")]
fn a_trigger_form_without_pull_request_names_none(#[case] source: &str) -> Result<()> {
    let triggers = parse(source)?;
    let events = trigger_events(&triggers);
    ensure!(
        !events.contains(&"pull_request"),
        "pull_request among {events:?}"
    );
    Ok(())
}

/// Scenario: the values `continue-on-error` can carry on a job or a step.
///
/// Invariant: `false` blocks, and everything else does not. `false` is what
/// GitHub does anyway, so a contract that called it non-blocking rejected a
/// job that propagates failure exactly as the policy requires. An expression is
/// decided at run time, so it cannot be proved false here and is refused.
///
/// Mutation proof (2026-09-15), each applied alone and reverted: judging every
/// present value non-blocking, as the contract did before, fails the two
/// blocking cases with `expected blocking`; judging every value false fails the
/// three non-blocking cases with `expected non-blocking`.
#[rstest]
#[case::boolean_false("continue-on-error: false", true)]
#[case::string_false("continue-on-error: \"false\"", true)]
#[case::boolean_true("continue-on-error: true", false)]
#[case::expression("continue-on-error: ${{ github.event_name == 'push' }}", false)]
#[case::string_true("continue-on-error: \"true\"", false)]
fn continue_on_error_blocks_only_when_it_is_false(
    #[case] source: &str,
    #[case] blocking: bool,
) -> Result<()> {
    let entry = parse_mapping(source)?;
    let reason = non_blocking(&entry);
    ensure!(
        reason.is_none() == blocking,
        "expected {} for {source}, got {reason:?}",
        if blocking { "blocking" } else { "non-blocking" }
    );
    Ok(())
}

/// Scenario: a value is asked whether it is provably false.
///
/// Invariant: only the boolean and its plain string spelling are. This is the
/// narrow half of the rule above, stated on its own so that widening
/// [`is_false`] to accept an expression fails here as well as there.
#[rstest]
#[case::boolean_false("false", true)]
#[case::string_false("\"false\"", true)]
#[case::boolean_true("true", false)]
#[case::string_off("\"off\"", false)]
#[case::expression("${{ false }}", false)]
fn only_a_written_false_is_provably_false(
    #[case] source: &str,
    #[case] expected: bool,
) -> Result<()> {
    let value = parse(source)?;
    ensure!(
        is_false(&value) == expected,
        "is_false({source}) should be {expected}"
    );
    Ok(())
}
