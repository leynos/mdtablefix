//! Drives the CV-005 readers and rules against fixtures directly.
//!
//! Every real workflow here complies, so a rule exercised only over them
//! passes whether or not it detects anything. Each case below builds the
//! breach it names and asserts the rule reports it, and the compliant
//! fixtures assert the rule stays quiet where it should.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use serde_yaml::Value;

use super::{reader, rules};

/// Parses a fixture through the contract's own strict reader.
fn parse(source: &str) -> Result<Value> { reader::parse("fixture", source) }

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

/// Scenario: a job calls a reusable workflow in another repository.
///
/// Invariant: it is not read as local, so the closure cannot claim a file
/// that happens to share the called workflow's name.
#[rstest]
#[case::remote("leynos/shared-actions/.github/workflows/called.yml@abc", None)]
#[case::nested("./.github/workflows/sub/called.yml", None)]
#[case::local("./.github/workflows/called.yml", Some("called.yml"))]
fn only_a_path_under_this_workflow_directory_is_local(
    #[case] reference: &str,
    #[case] expected: Option<&str>,
) {
    assert_eq!(reader::local_call(reference), expected, "for {reference}");
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

/// Scenario: push triggers that name other branches, tags, or none.
///
/// Invariant: only a push restricted to `main` is the publisher.
#[rstest]
#[case::main_only("on:\n  push:\n    branches: [main]\njobs: {}\n", true)]
#[case::unfiltered("on:\n  push:\njobs: {}\n", false)]
#[case::scalar("on: push\njobs: {}\n", false)]
#[case::tags("on:\n  push:\n    tags: ['v*']\njobs: {}\n", false)]
#[case::another_branch("on:\n  push:\n    branches: [develop]\njobs: {}\n", false)]
#[case::main_and_another("on:\n  push:\n    branches: [main, develop]\njobs: {}\n", false)]
fn only_a_push_restricted_to_main_is_the_publisher(
    #[case] source: &str,
    #[case] expected: bool,
) -> Result<()> {
    ensure!(
        rules::publishes_from_main(&parse(source)?) == expected,
        "expected {expected} for {source:?}"
    );
    Ok(())
}

/// A publisher, with the three pieces the cases vary left as placeholders.
const PUBLISHER: &str = r#"
on:
  push:
    branches: [main]
  workflow_dispatch:
@CONCURRENCY@
jobs:
  coverage:
    steps:
      - uses: leynos/shared-actions/.github/actions/generate-coverage@abc
        with:
          with-ratchet: 'true'
@EXTRA_STEP@
      - env:
          CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}
        if: "@UPLOAD_IF@"
        uses: leynos/shared-actions/.github/actions/upload-codescene-coverage@abc
"#;

/// Renders [`PUBLISHER`] with the pieces a case varies.
fn publisher(concurrency: &str, upload_if: &str, extra_step: &str) -> String {
    PUBLISHER
        .replace("@CONCURRENCY@", concurrency)
        .replace("@UPLOAD_IF@", upload_if)
        .replace("@EXTRA_STEP@", extra_step)
}

/// The publisher's concurrency block as this repository writes it.
const QUEUE: &str = "concurrency:\n  group: pub\n  cancel-in-progress: false";
/// The upload condition as this repository writes it.
const GUARD: &str = "${{ env.CS_ACCESS_TOKEN != '' && github.ref == 'refs/heads/main' }}";

/// Scenario: a publisher is varied one clause at a time.
///
/// Invariant: each variation is reported by the clause it breaks, and the
/// unvaried publisher has no findings. The appended disjunction is the
/// mutation a substring check passes. The prepended one is the case only the
/// refusal of `||` catches: `&&` binds tighter, so the ref check survives as
/// an exact conjunct of the split while the leading disjunct uploads a
/// dispatch from any branch on its own.
#[rstest]
#[case::complies(QUEUE, GUARD, "", None)]
#[case::disjunction_appended(
    QUEUE,
    "${{ env.CS_ACCESS_TOKEN != '' && github.ref == 'refs/heads/main' || github.event_name == \
     'workflow_dispatch' }}",
    "",
    Some("not guarded")
)]
#[case::disjunction_prepended(
    QUEUE,
    "${{ github.event_name == 'workflow_dispatch' || env.CS_ACCESS_TOKEN != '' && github.ref == \
     'refs/heads/main' }}",
    "",
    Some("not guarded")
)]
#[case::no_ref_guard(QUEUE, "${{ env.CS_ACCESS_TOKEN != '' }}", "", Some("not guarded"))]
#[case::cancels(
    "concurrency:\n  group: pub\n  cancel-in-progress: true",
    GUARD,
    "",
    Some("cancel")
)]
#[case::cancels_by_expression(
    "concurrency:\n  group: pub\n  cancel-in-progress: ${{ true }}",
    GUARD,
    "",
    Some("cancel")
)]
#[case::no_group("", GUARD, "", Some("no concurrency group"))]
#[case::token_elsewhere(
    QUEUE,
    GUARD,
    "      - run: echo ${{ secrets.CS_ACCESS_TOKEN }}",
    Some("other than the upload")
)]
fn the_publisher_rule_names_the_clause_broken(
    #[case] concurrency: &str,
    #[case] upload_if: &str,
    #[case] extra_step: &str,
    #[case] expected: Option<&str>,
) -> Result<()> {
    let source = publisher(concurrency, upload_if, extra_step);
    let findings = rules::publisher_findings(&parse(&source)?);
    match expected {
        None => ensure!(findings.is_empty(), "unexpected findings: {findings:?}"),
        Some(clause) => ensure!(
            findings.len() == 1 && findings[0].contains(clause),
            "expected one finding naming {clause:?}, saw {findings:?}"
        ),
    }
    Ok(())
}

/// The upload step's token, as [`PUBLISHER`] writes it.
const UPLOAD_TOKEN: &str =
    "      - env:\n          CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}\n";
/// A scope-wide declaration of the token, indented for the workflow root.
const WIDE_TOKEN: &str = "env:\n  CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}\n";

/// Scenario: the token is moved off the upload step, or declared more widely.
///
/// Invariant: each placement is named. Moving the token to the coverage step
/// satisfies "some step holds it" while the upload's own guard goes false and
/// publishing silently stops, and a workflow- or job-level `env` hands it to
/// every step, so neither may pass as the upload holding it.
#[rstest]
#[case::moved_to_coverage(
    |source: String| source
        .replace(UPLOAD_TOKEN, "      - env: {}\n")
        .replace("        with:\n          with-ratchet", "        env:\n          CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}\n        with:\n          with-ratchet"),
    &["does not receive", "other than the upload"][..],
)]
#[case::workflow_env(|source: String| source.replace("jobs:\n", &format!("{WIDE_TOKEN}jobs:\n")), &["for every job"][..])]
#[case::job_env(
    |source: String| source.replace("    steps:\n", &format!("    {}    steps:\n", WIDE_TOKEN.replace("\n  ", "\n      "))),
    &["for every step"][..],
)]
fn the_token_sits_on_the_upload_alone(
    #[case] vary: fn(String) -> String,
    #[case] expected: &[&str],
) -> Result<()> {
    let source = vary(publisher(QUEUE, GUARD, ""));
    let findings = rules::publisher_findings(&parse(&source)?);
    ensure!(
        findings.len() == expected.len()
            && expected
                .iter()
                .all(|clause| findings.iter().any(|f| f.contains(clause))),
        "expected findings naming {expected:?}, saw {findings:?}"
    );
    Ok(())
}

/// Scenario: a publisher that runs the upload action in `check` mode.
///
/// Invariant: that is not an upload, so the omission is reported.
#[test]
fn check_mode_is_not_an_upload() -> Result<()> {
    let source = publisher(QUEUE, GUARD, "").replace(
        "upload-codescene-coverage@abc\n",
        "upload-codescene-coverage@abc\n        with:\n          mode: check\n",
    );
    let findings = rules::publisher_findings(&parse(&source)?);
    ensure!(
        findings.iter().any(|f| f.contains("uploads nothing")),
        "check mode read as an upload: {findings:?}"
    );
    Ok(())
}

/// Scenario: conditions whose operators sit inside quoted literals.
///
/// Invariant: a `||` inside a string is not a disjunction, and an `&&`
/// inside one does not split a conjunct.
#[rstest]
#[case::quoted_or("${{ github.ref == 'refs/heads/main' && env.X != 'a||b' }}", Some(2))]
#[case::quoted_and("${{ github.ref == 'refs/heads/main' && env.X != 'a&&b' }}", Some(2))]
#[case::bare_or("github.ref == 'refs/heads/main' || true", None)]
fn quoted_operators_are_not_operators(#[case] condition: &str, #[case] expected: Option<usize>) {
    assert_eq!(
        rules::conjuncts(condition).map(|parts| parts.len()),
        expected,
        "for {condition}"
    );
}
