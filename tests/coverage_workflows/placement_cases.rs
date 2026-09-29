//! Drives the runner-placement judgement over constructed expressions and
//! workflows, then applies it to the real files.
//!
//! A check parametrized over this repository's own correct workflow passes
//! whether or not it discriminates anything, so the judgement is driven
//! directly in both directions first: the estate expression must pass, and
//! each way of misplacing a lane must fail.

use anyhow::{Result, ensure};
use rstest::rstest;

use super::{
    placement::{self, Origin, Placed},
    pull_request_cases::parse,
    reader,
};

/// The estate's runner expression, as a lane writes it.
const ESTATE: &str =
    "${{ github.event.pull_request.head.repo.fork && 'ubuntu-latest' || 'ubicloud-standard-4' }}";

/// Every job that can land on Ubicloud, with the ceiling it states in minutes.
///
/// The inventory is exact, so a new Ubicloud lane without a ceiling, or a
/// ceiling removed or changed, fails here until the change is reviewed.
const CEILINGS: [(&str, &str, u64); 1] = [("coverage-main.yml", "coverage-upload", 30)];

/// Scenario: the estate expression is evaluated for each kind of run.
///
/// Invariant: a push, a dispatch and a same-repository pull request select
/// Ubicloud, and only a fork's pull request selects the hosted pool.
#[rstest]
#[case::push_or_dispatch(Origin::NoPullRequest, "ubicloud-standard-4")]
#[case::same_repository(Origin::SameRepository, "ubicloud-standard-4")]
#[case::fork(Origin::Fork, "ubuntu-latest")]
fn the_estate_expression_places_each_run(#[case] origin: Origin, #[case] wanted: &str) {
    assert_eq!(
        placement::selected_runner(ESTATE, origin).as_deref(),
        Some(wanted)
    );
}

/// Scenario: an expression misplaces a lane in one of the ways a careless
/// edit would.
///
/// Invariant: each is reported, and the estate expression is not.
#[rstest]
#[case::estate(ESTATE, 0)]
#[case::always_hosted("ubuntu-latest", 3)]
#[case::always_ubicloud("ubicloud-standard-4", 3)]
#[case::inverted_arms(
    "${{ github.event.pull_request.head.repo.fork && 'ubicloud-standard-4' || 'ubuntu-latest' }}",
    3
)]
#[case::another_label(
    "${{ github.event.pull_request.head.repo.fork && 'ubuntu-latest' || 'ubicloud-standard-2' }}",
    2
)]
#[case::another_condition(
    "${{ github.event_name == 'pull_request' && 'ubuntu-latest' || 'ubicloud-standard-4' }}",
    3
)]
#[case::fork_kept_on_ubicloud(
    concat!(
        "${{ github.event.pull_request.head.repo.fork && 'ubicloud-standard-4' ",
        "|| 'ubicloud-standard-4' }}"
    ),
    1
)]
fn a_misplaced_lane_is_reported(#[case] runs_on: &str, #[case] expected: usize) -> Result<()> {
    let faults = placement::placement_faults(runs_on);
    ensure!(
        faults.len() == expected,
        "expected {expected}, saw {faults:?}"
    );
    Ok(())
}

/// Scenario: constructed workflows state their ceiling in each way.
///
/// Invariant: only a whole number of minutes is read as a ceiling, so a
/// missing key and a string both read as none and the inventory then fails.
#[rstest]
#[case::stated("    timeout-minutes: 30\n", Some(30))]
#[case::missing("", None)]
#[case::a_string("    timeout-minutes: thirty\n", None)]
#[case::an_expression("    timeout-minutes: ${{ inputs.ceiling }}\n", None)]
fn a_ceiling_is_read_only_when_it_is_a_number(
    #[case] key: &str,
    #[case] expected: Option<u64>,
) -> Result<()> {
    let source = format!("on: push\njobs:\n  lane:\n    runs-on: {ESTATE}\n{key}");
    let all: reader::Workflows = [("x.yml".to_owned(), parse(&source)?)].into();
    let placed = placement::placed_jobs(&all);
    ensure!(
        placed
            == [Placed {
                workflow: "x.yml".to_owned(),
                job: "lane".to_owned(),
                ceiling: expected
            }],
        "read {placed:?}"
    );
    Ok(())
}

/// Scenario: the repository's own workflows are read.
///
/// Invariant: exactly the inventoried jobs can land on Ubicloud, each states
/// its ceiling, and each selects its runner by the estate expression.
#[test]
fn every_ubicloud_lane_is_placed_by_the_estate_expression_and_states_a_ceiling() -> Result<()> {
    let all = reader::workflows()?;
    let found: Vec<(String, String, Option<u64>)> = placement::placed_jobs(&all)
        .into_iter()
        .map(|placed| (placed.workflow, placed.job, placed.ceiling))
        .collect();
    let expected: Vec<(String, String, Option<u64>)> = CEILINGS
        .iter()
        .map(|(workflow, job, ceiling)| ((*workflow).to_owned(), (*job).to_owned(), Some(*ceiling)))
        .collect();
    ensure!(found == expected, "found {found:?}, expected {expected:?}");
    for (lane, runs_on) in placement::placement_expressions(&all) {
        let faults = placement::placement_faults(&runs_on);
        ensure!(faults.is_empty(), "{lane} is misplaced: {faults:?}");
    }
    Ok(())
}

/// Scenario: a job names an Ubicloud runner in a sequence or a mapping rather
/// than as a string.
///
/// Invariant: it is inventoried and judged, and the judgement rejects it, so
/// no shape of `runs-on` places a lane outside the estate expression.
#[rstest]
#[case::sequence("[ubicloud-standard-4]")]
#[case::mapping("{ group: ubicloud-standard-4 }")]
fn a_non_scalar_ubicloud_runner_is_inventoried_and_rejected(#[case] runs_on: &str) -> Result<()> {
    let source = format!("on: push\njobs:\n  lane:\n    runs-on: {runs_on}\n");
    let all: reader::Workflows = [("x.yml".to_owned(), parse(&source)?)].into();
    ensure!(placement::placed_jobs(&all).len() == 1, "not inventoried");
    let expressions = placement::placement_expressions(&all);
    ensure!(
        expressions
            .iter()
            .all(|(_, text)| !placement::placement_faults(text).is_empty())
            && expressions.len() == 1,
        "not rejected: {expressions:?}"
    );
    Ok(())
}
