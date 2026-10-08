//! Drives matrix placement over several Ubicloud rows at once.
//!
//! The single-row cases in [`super::placement_cases`] cannot tell an
//! implementation that returns every Ubicloud row from one that stops at the
//! first, because each fixture has only one. These cases put several rows with
//! different expressions in one matrix, in both the `include` form and the
//! top-level list form, and then generate the matrices.

use anyhow::{Result, ensure};
use proptest::prelude::*;
use rstest::rstest;

use super::{placement, reader};

/// The estate's runner expression, as a lane writes it.
const ESTATE: &str =
    "${{ github.event.pull_request.head.repo.fork && 'ubuntu-latest' || 'ubicloud-standard-2' }}";
/// A literal Ubicloud label, which no runner-selection expression places.
const LITERAL: &str = "ubicloud-standard-2";
/// A hosted label, which is not a placement.
const HOSTED: &str = "ubuntu-latest";

/// Returns every placement a one-job workflow reads, with the faults each has.
fn judged(source: &str) -> Result<Vec<(String, usize)>> {
    let all: reader::Workflows = [("x.yml".to_owned(), reader::parse("fixture", source)?)].into();
    Ok(placement::placement_expressions(&all)
        .into_iter()
        .map(|(_, text)| {
            let faults = placement::placement_faults(&text, "ubicloud-standard-2").len();
            (text, faults)
        })
        .collect())
}

/// Scenario: one matrix gives a valid estate row, a hosted row and an invalid
/// literal Ubicloud row, as `include` rows or as a top-level list.
///
/// Invariant: both Ubicloud rows are returned, in order, the hosted row is not,
/// the estate row has no faults and the literal row has some.
#[rstest]
#[case::include_rows(format!(
    "on: push\njobs:\n  lane:\n    runs-on: ${{{{ matrix.runner }}}}\n    strategy:\n      matrix:\n        include:\n          - runner: \"{ESTATE}\"\n          - runner: {HOSTED}\n          - runner: {LITERAL}\n"
))]
#[case::top_level_list(format!(
    "on: push\njobs:\n  lane:\n    runs-on: ${{{{ matrix.runner }}}}\n    strategy:\n      matrix:\n        runner:\n          - \"{ESTATE}\"\n          - {HOSTED}\n          - {LITERAL}\n"
))]
fn every_ubicloud_row_is_returned_and_judged(#[case] source: String) -> Result<()> {
    let placements = judged(&source)?;
    ensure!(
        placements.len() == 2,
        "expected both Ubicloud rows: {placements:?}"
    );
    ensure!(
        placements[0] == (ESTATE.to_owned(), 0),
        "estate row: {:?}",
        placements[0]
    );
    ensure!(
        placements[1].0 == LITERAL && placements[1].1 > 0,
        "the invalid row was not rejected: {:?}",
        placements[1]
    );
    Ok(())
}

/// How one generated matrix row is placed.
#[derive(Clone, Copy, Debug)]
enum Row {
    Hosted,
    Estate,
    Literal,
}

impl Row {
    const fn value(self) -> &'static str {
        match self {
            Self::Hosted => HOSTED,
            Self::Estate => ESTATE,
            Self::Literal => LITERAL,
        }
    }

    const fn places(self) -> bool { !matches!(self, Self::Hosted) }
}

fn rows() -> impl Strategy<Value = Vec<Row>> {
    prop::collection::vec(
        prop_oneof![Just(Row::Hosted), Just(Row::Estate), Just(Row::Literal)],
        1..8,
    )
}

fn matrix_source(read_key: &str, rows_key: &str, rows: &[Row]) -> String {
    let mut source = format!(
        "on: push\njobs:\n  lane:\n    runs-on: ${{{{ matrix.{read_key} }}}}\n    strategy:\n      matrix:\n        include:\n"
    );
    source.extend(
        rows.iter()
            .map(|row| format!("          - {rows_key}: \"{}\"\n", row.value())),
    );
    source
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Scenario: a matrix of generated rows, read through a generated key.
    ///
    /// Invariant: the placements are exactly the Ubicloud rows, in row order,
    /// each judged on its own value; hosted rows are excluded; adding or
    /// reordering rows changes only which placements appear and where.
    #[test]
    fn placements_are_exactly_the_ubicloud_rows_under_the_read_key(
        key in "k[a-z0-9_]{0,8}",
        rows in rows(),
    ) {
        let found = judged(&matrix_source(&key, &key, &rows)).expect("fixture parses");
        let wanted: Vec<(String, usize)> = rows
            .iter()
            .filter(|row| row.places())
            .map(|row| (row.value().to_owned(), usize::from(matches!(row, Row::Literal)) * 3))
            .collect();
        prop_assert_eq!(found, wanted);
    }

    /// Scenario: the Ubicloud rows sit under a key the `runs-on` does not read.
    ///
    /// Invariant: nothing is attributed to a row, and when any row names
    /// Ubicloud the job is still inventoried with its own `runs-on` text, which
    /// no runner-selection expression places and so is rejected.
    #[test]
    fn rows_under_an_unread_key_are_never_returned_as_placements(
        key in "k[a-z0-9_]{0,8}",
        rows in rows(),
    ) {
        let found = judged(&matrix_source(&key, "unread", &rows)).expect("fixture parses");
        if rows.iter().any(|row| row.places()) {
            prop_assert_eq!(found.len(), 1);
            prop_assert!(found[0].0.contains("matrix."));
            prop_assert!(found[0].1 > 0);
        } else {
            prop_assert!(found.is_empty());
        }
    }
}
