//! Drives the runner-placement judgement over constructed expressions and
//! workflows, then applies it to the real files.
//!
//! A check parametrized over this repository's own correct workflow passes
//! whether or not it discriminates anything, so the judgement is driven
//! directly in both directions first: the runner-selection expression must pass, and
//! each way of misplacing a lane must fail.

use anyhow::{Result, ensure};
use rstest::rstest;
use serde_yaml::Value;

use super::{
    placement::{self, Origin, Placed},
    reader,
};

/// Parses a fixture through the same reader the real workflows use.
fn parse(source: &str) -> Result<Value> { reader::parse("fixture", source) }

/// The estate's runner expression, as a lane writes it.
const ESTATE: &str =
    "${{ github.event.pull_request.head.repo.fork && 'ubuntu-latest' || 'ubicloud-standard-2' }}";

/// Every job that can land on Ubicloud, with the runner class it names and the
/// ceiling it states in minutes.
///
/// The inventory is exact, so a new Ubicloud lane without a ceiling, or a class
/// or ceiling changed, fails here until the change is reviewed.
const PLACEMENTS: [(&str, &str, &str, u64); 3] = [
    ("ci.yml", "build-test", "ubicloud-standard-4", 15),
    ("ci.yml", "binstall-packaging", "ubicloud-standard-2", 10),
    (
        "coverage-main.yml",
        "coverage-upload",
        "ubicloud-standard-4",
        10,
    ),
];

/// Scenario: the runner-selection expression is evaluated for each kind of run.
///
/// Invariant: a push, a dispatch and a same-repository pull request select
/// Ubicloud, and only a fork's pull request selects the hosted pool.
#[rstest]
#[case::push_or_dispatch(Origin::NoPullRequest, "ubicloud-standard-2")]
#[case::same_repository(Origin::SameRepository, "ubicloud-standard-2")]
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
/// Invariant: each is reported, and the runner-selection expression is not.
#[rstest]
#[case::estate(ESTATE, 0)]
#[case::always_hosted("ubuntu-latest", 3)]
#[case::always_ubicloud("ubicloud-standard-2", 3)]
#[case::inverted_arms(
    "${{ github.event.pull_request.head.repo.fork && 'ubicloud-standard-2' || 'ubuntu-latest' }}",
    3
)]
#[case::another_label(
    "${{ github.event.pull_request.head.repo.fork && 'ubuntu-latest' || 'ubicloud-standard-4' }}",
    2
)]
#[case::another_condition(
    "${{ github.event_name == 'pull_request' && 'ubuntu-latest' || 'ubicloud-standard-2' }}",
    3
)]
#[case::fork_kept_on_ubicloud(
    concat!(
        "${{ github.event.pull_request.head.repo.fork && 'ubicloud-standard-2' ",
        "|| 'ubicloud-standard-2' }}"
    ),
    1
)]
fn a_misplaced_lane_is_reported(#[case] runs_on: &str, #[case] expected: usize) -> Result<()> {
    let faults = placement::placement_faults(runs_on, "ubicloud-standard-2");
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
/// its ceiling, and each selects its runner by the runner-selection expression.
#[test]
fn every_ubicloud_lane_is_placed_by_the_estate_expression_and_states_a_ceiling() -> Result<()> {
    let all = reader::workflows()?;
    let found: Vec<(String, String, Option<u64>)> = placement::placed_jobs(&all)
        .into_iter()
        .map(|placed| (placed.workflow, placed.job, placed.ceiling))
        .collect();
    let expected: Vec<(String, String, Option<u64>)> = PLACEMENTS
        .iter()
        .map(|(workflow, job, _, ceiling)| {
            ((*workflow).to_owned(), (*job).to_owned(), Some(*ceiling))
        })
        .collect();
    ensure!(found == expected, "found {found:?}, expected {expected:?}");
    let expressions = placement::placement_expressions(&all);
    for (lane, runs_on) in &expressions {
        let label = PLACEMENTS
            .iter()
            .find(|(workflow, job, ..)| format!("{workflow}: {job}") == *lane)
            .map(|(_, _, label, _)| *label);
        ensure!(label.is_some(), "{lane} is not in the inventory");
        let faults = placement::placement_faults(runs_on, label.unwrap_or_default());
        ensure!(faults.is_empty(), "{lane} is misplaced: {faults:?}");
    }
    let lanes: Vec<&String> = expressions.iter().map(|(lane, _)| lane).collect();
    for (workflow, job, ..) in PLACEMENTS {
        ensure!(
            lanes.contains(&&format!("{workflow}: {job}")),
            "{workflow}: {job} names no Ubicloud runner"
        );
    }
    Ok(())
}

/// Scenario: a job names an Ubicloud runner in a sequence or a mapping rather
/// than as a string.
///
/// Invariant: it is inventoried and judged, and the judgement rejects it, so
/// no shape of `runs-on` places a lane outside the runner-selection expression.
#[rstest]
#[case::sequence("[ubicloud-standard-2]")]
#[case::mapping("{ group: ubicloud-standard-2 }")]
fn a_non_scalar_ubicloud_runner_is_inventoried_and_rejected(#[case] runs_on: &str) -> Result<()> {
    let source = format!("on: push\njobs:\n  lane:\n    runs-on: {runs_on}\n");
    let all: reader::Workflows = [("x.yml".to_owned(), parse(&source)?)].into();
    ensure!(placement::placed_jobs(&all).len() == 1, "not inventoried");
    let expressions = placement::placement_expressions(&all);
    ensure!(
        expressions
            .iter()
            .all(|(_, text)| !placement::placement_faults(text, "ubicloud-standard-2").is_empty())
            && expressions.len() == 1,
        "not rejected: {expressions:?}"
    );
    Ok(())
}

/// Scenario: a lane names a different Ubicloud runner class from the one the
/// inventory records.
///
/// Invariant: it is reported in both directions, so a lane moved between
/// `standard-2` and `standard-4` fails until the inventory says so.
#[rstest]
#[case::inventory_says_four("ubicloud-standard-4", 0)]
#[case::inventory_says_two("ubicloud-standard-2", 2)]
fn a_lane_on_another_runner_class_is_reported(#[case] label: &str, #[case] expected: usize) {
    let four = ESTATE.replace("standard-2", "standard-4");
    assert_eq!(placement::placement_faults(&four, label).len(), expected);
}

/// A job whose `runs-on` reads a matrix value, with one row placed by the given
/// runner value and the others hosted or native.
fn matrix_job(reference: &str, ubicloud_row: &str) -> String {
    format!(
        concat!(
            "on: push\njobs:\n  lane:\n    runs-on: {reference}\n",
            "    strategy:\n      matrix:\n        include:\n",
            "          - runner: {row}\n            os: linux\n",
            "          - runner: macos-15\n            os: macos\n",
        ),
        reference = reference,
        row = ubicloud_row,
    )
}

/// Scenario: a matrix job places its Linux row on Ubicloud through a runner
/// value, and reads that value as `matrix.runner` or `matrix['runner']`.
///
/// Invariant: the row is inventoried and judged on its own expression, the
/// hosted macOS row is left alone, and the judgement passes the runner-selection
/// expression and rejects a literal Ubicloud label, an inverted pair and another
/// class, in both access forms.
#[rstest]
#[case::dotted_estate("${{ matrix.runner }}", ESTATE, 0)]
#[case::indexed_estate("${{ matrix['runner'] }}", ESTATE, 0)]
#[case::literal_label("${{ matrix.runner }}", "ubicloud-standard-2", 3)]
#[case::inverted(
    "${{ matrix.runner }}",
    "${{ github.event.pull_request.head.repo.fork && 'ubicloud-standard-2' || 'ubuntu-latest' }}",
    3
)]
#[case::another_class(
    "${{ matrix.runner }}",
    "${{ github.event.pull_request.head.repo.fork && 'ubuntu-latest' || 'ubicloud-standard-4' }}",
    2
)]
fn a_matrix_row_is_judged_on_its_own_expression(
    #[case] reference: &str,
    #[case] row: &str,
    #[case] expected: usize,
) -> Result<()> {
    let source = matrix_job(reference, &format!("\"{row}\""));
    let all: reader::Workflows = [("x.yml".to_owned(), parse(&source)?)].into();
    ensure!(placement::placed_jobs(&all).len() == 1, "not inventoried");
    let expressions = placement::placement_expressions(&all);
    ensure!(
        expressions.len() == 1,
        "expected the one Ubicloud row: {expressions:?}"
    );
    let faults = placement::placement_faults(&expressions[0].1, "ubicloud-standard-2");
    ensure!(
        faults.len() == expected,
        "expected {expected}, saw {faults:?}"
    );
    Ok(())
}

/// Scenario: a matrix job with only hosted and native rows.
///
/// Invariant: it is not inventoried, so a matrix that never names Ubicloud
/// needs no ceiling.
#[test]
fn a_matrix_of_hosted_rows_is_not_inventoried() -> Result<()> {
    let source = matrix_job("${{ matrix.runner }}", "ubuntu-latest");
    let all: reader::Workflows = [("x.yml".to_owned(), parse(&source)?)].into();
    ensure!(
        placement::placed_jobs(&all).is_empty(),
        "a hosted matrix was inventoried"
    );
    Ok(())
}

/// Scenario: the matrix names Ubicloud under a key the `runs-on` does not read.
///
/// Invariant: the job is inventoried and its `runs-on` text is rejected, since
/// no runner-selection expression places it.
#[test]
fn a_matrix_naming_ubicloud_elsewhere_is_inventoried_and_rejected() -> Result<()> {
    let source = concat!(
        "on: push\njobs:\n  lane:\n    runs-on: ${{ matrix.os }}\n",
        "    strategy:\n      matrix:\n        include:\n",
        "          - os: ubuntu-latest\n            runner: ubicloud-standard-2\n",
    );
    let all: reader::Workflows = [("x.yml".to_owned(), parse(source)?)].into();
    ensure!(placement::placed_jobs(&all).len() == 1, "not inventoried");
    let expressions = placement::placement_expressions(&all);
    ensure!(
        expressions.len() == 1,
        "expected one entry: {expressions:?}"
    );
    ensure!(
        !placement::placement_faults(&expressions[0].1, "ubicloud-standard-2").is_empty(),
        "not rejected: {expressions:?}"
    );
    Ok(())
}
