//! Drives the pull-request readers and rules against fixtures directly.
//!
//! Every real workflow here complies, so a rule exercised only over them
//! passes whether or not it detects anything. Each case below builds the
//! breach it names and asserts the rule reports it, and the compliant
//! fixtures assert the rule stays quiet where it should. The publisher's
//! cases are in `publisher_cases.rs`.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_yaml::Value;

use super::{reader, rules};

/// Parses a fixture through the contract's own strict reader.
pub(super) fn parse(source: &str) -> Result<Value> { reader::parse("fixture", source) }

/// A pull-request workflow that breaches every clause at once.
const BREACHING_PULL_REQUEST: &str = r"
on:
  pull_request:
jobs:
  build:
    steps:
      - uses: leynos/shared-actions/.github/actions/generate-coverage@abc
        with:
          output-path: lcov.info
      - env:
          CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}
        uses: leynos/shared-actions/.github/actions/upload-codescene-coverage@abc
      - run: cs-coverage upload --format lcov
      - run: curl https://api.codescene.io/v2/projects
";

/// The same workflow with every breach removed.
const COMPLIANT_PULL_REQUEST: &str = r"
on:
  pull_request:
jobs:
  build:
    steps:
      - uses: leynos/shared-actions/.github/actions/generate-coverage@abc
        with:
          output-path: lcov.info
          with-ratchet: 'true'
          publish-artefact: 'false'
";

/// Scenario: the pull-request rule meets every breach, then none.
///
/// Invariant: token, host, action, CLI, missing ratchet and published report
/// are each reported, six findings in all, and the compliant twin has none.
#[rstest]
#[case::breaching(BREACHING_PULL_REQUEST, 6)]
#[case::compliant(COMPLIANT_PULL_REQUEST, 0)]
fn the_pull_request_rule_reports_what_it_should(
    #[case] source: &str,
    #[case] expected: usize,
) -> Result<()> {
    let findings = rules::pull_request_findings(&parse(source)?);
    ensure!(
        findings.len() == expected,
        "expected {expected}, saw {findings:?}"
    );
    Ok(())
}

/// Scenario: the token reaches a pull-request job by each route GitHub offers.
///
/// Invariant: a reference in a `run` body, an action input, an `env` value,
/// a named `secrets:` forwarding, and a blanket `secrets: inherit` are each
/// reported. The last names no secret at all, which is why it is its own
/// clause rather than a search for the name.
#[rstest]
#[case::run_body("steps:\n  - run: echo ${{ secrets.CS_ACCESS_TOKEN }}\n")]
#[case::action_input(
    "steps:\n  - uses: x/y@abc\n    with:\n      token: ${{ secrets.CS_ACCESS_TOKEN }}\n"
)]
#[case::env_value("env:\n  T: ${{ secrets.CS_ACCESS_TOKEN }}\nsteps:\n  - run: 'true'\n")]
#[case::named_forwarding(
    "uses: ./.github/workflows/c.yml\nsecrets:\n  CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN \
     }}\n"
)]
#[case::inherit("uses: ./.github/workflows/c.yml\nsecrets: inherit\n")]
fn every_route_to_the_token_is_reported(#[case] job: &str) -> Result<()> {
    let indented: String = job
        .lines()
        .map(|line| ["    ", line, "\n"].concat())
        .collect();
    let source = format!("on: pull_request\njobs:\n  lane:\n{indented}");
    let findings = rules::pull_request_findings(&parse(&source)?);
    ensure!(!findings.is_empty(), "the route was not reported: {source}");
    Ok(())
}

/// Scenario: a pull-request job calls a `workflow_call`-only workflow with
/// `secrets: inherit`, and the callee curls `codescene.io` with the token.
///
/// Invariant: the callee is in the closure however the call is spelled, and
/// the rules report it. This is the probe that found the hole: the callee
/// names no pull request, so a trigger-only enumeration never read it.
#[rstest]
#[case::dot_prefixed("./.github/workflows/called.yml")]
#[case::bare(".github/workflows/called.yml")]
fn a_called_workflow_is_inside_the_pull_request_closure(#[case] call: &str) -> Result<()> {
    let caller =
        format!("on: [pull_request]\njobs:\n  call:\n    uses: {call}\n    secrets: inherit\n");
    let callee = r#"
on: workflow_call
jobs:
  leak:
    steps:
      - run: |
          curl -H "Authorization: ${{ secrets.CS_ACCESS_TOKEN }}" https://codescene.io/api
"#;
    let all: reader::Workflows = [
        ("caller.yml".to_owned(), parse(&caller)?),
        ("called.yml".to_owned(), parse(callee)?),
    ]
    .into();
    let closure = reader::pull_request_closure(&all);
    ensure!(
        closure.contains("called.yml"),
        "the callee escaped: {closure:?}"
    );
    let callee_findings = rules::pull_request_findings(all.get("called.yml").context("callee")?);
    ensure!(
        callee_findings.len() == 2,
        "expected token and host, saw {callee_findings:?}"
    );
    Ok(())
}

/// Scenario: job-level `uses` references in each spelling.
///
/// Invariant: `./` and GitHub's documented `$/` both reach a local file;
/// another repository's workflow is remote; and a local-shaped reference that
/// resolves to no single file here, one carrying an `@ref` or naming a
/// subdirectory, is refused rather than read as remote, where whatever it runs
/// would escape the closure.
#[rstest]
#[case::dot_prefixed("./.github/workflows/called.yml", reader::Call::Local("called.yml"))]
#[case::dollar_prefixed("$/.github/workflows/called.yml", reader::Call::Local("called.yml"))]
#[case::remote(
    "leynos/shared-actions/.github/workflows/called.yml@abc",
    reader::Call::Remote
)]
#[case::dollar_with_ref("$/.github/workflows/called.yml@main", reader::Call::Refused)]
#[case::dot_with_ref("./.github/workflows/called.yml@main", reader::Call::Refused)]
#[case::nested("./.github/workflows/sub/called.yml", reader::Call::Refused)]
fn each_call_spelling_is_classified(#[case] reference: &str, #[case] expected: reader::Call) {
    assert_eq!(
        reader::classify_call(reference),
        expected,
        "for {reference}"
    );
}

/// Scenario: a refused call sits in a pull-request workflow.
///
/// Invariant: the pull-request rule reports it, naming the job and the
/// reference.
#[test]
fn a_refused_call_is_a_finding() -> Result<()> {
    let source = "on: pull_request\njobs:\n  call:\n    uses: $/.github/workflows/c.yml@main\n";
    let findings = rules::pull_request_findings(&parse(source)?);
    ensure!(
        findings.len() == 1 && findings[0].contains("resolves to no workflow"),
        "expected one refused call, saw {findings:?}"
    );
    Ok(())
}

/// Scenario: a pull request reaches the host two calls deep, through a
/// middle workflow that is itself only `workflow_call`.
///
/// Invariant: the closure is transitive. A traversal that followed one hop
/// would take in the middle workflow, which is clean, and stop before the one
/// that contacts `codescene.io`.
#[test]
fn the_closure_follows_a_chain_of_calls() -> Result<()> {
    let caller = "on: pull_request\njobs:\n  first:\n    uses: ./.github/workflows/middle.yml\n";
    let middle = "on: workflow_call\njobs:\n  second:\n    uses: $/.github/workflows/leaf.yml\n";
    let leaf = "on: workflow_call\njobs:\n  leak:\n    steps:\n      - run: curl https://codescene.io/api\n";
    let all: reader::Workflows = [
        ("caller.yml".to_owned(), parse(caller)?),
        ("middle.yml".to_owned(), parse(middle)?),
        ("leaf.yml".to_owned(), parse(leaf)?),
    ]
    .into();
    let closure = reader::pull_request_closure(&all);
    ensure!(
        closure.contains("middle.yml") && closure.contains("leaf.yml"),
        "the chain was not followed: {closure:?}"
    );
    let leaf_findings = rules::pull_request_findings(all.get("leaf.yml").context("leaf")?);
    ensure!(
        leaf_findings.len() == 1,
        "expected the host, saw {leaf_findings:?}"
    );
    Ok(())
}

/// Scenario: a pull-request trigger in each form GitHub accepts.
///
/// Invariant: every one is read as a pull request. A mapping-only reader
/// turns the sequence into one key named after the whole list, and the
/// workflow escapes every pull-request clause.
#[rstest]
#[case::scalar("on: pull_request\njobs: {}\n")]
#[case::sequence("on: [push, pull_request]\njobs: {}\n")]
#[case::mapping("on:\n  pull_request:\njobs: {}\n")]
#[case::quoted_key("'on':\n  pull_request:\njobs: {}\n")]
#[case::boolean_key("true:\n  pull_request:\njobs: {}\n")]
#[case::target("on: [pull_request_target]\njobs: {}\n")]
fn every_trigger_form_is_read(#[case] source: &str) -> Result<()> {
    ensure!(
        reader::starts_on_pull_request(&parse(source)?),
        "not read as a pull request: {source:?}"
    );
    Ok(())
}

/// Scenario: a workflow declares the same key twice in one mapping.
///
/// Invariant: the reader refuses it. A parser keeping the last duplicate
/// would let a lane carry one `runs-on` in the file and another in the parse.
#[test]
fn a_duplicate_key_is_refused() {
    let source = "on: pull_request\njobs:\n  a:\n    runs-on: x\n    runs-on: y\n";
    let Err(error) = reader::parse("fixture", source) else {
        panic!("a duplicate key was accepted");
    };
    assert!(
        format!("{error:#}").contains("duplicate entry"),
        "refused for another reason: {error:#}"
    );
}
