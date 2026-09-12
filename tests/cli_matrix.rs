//! Matrix tests for CLI option interactions.
//!
//! The catalogue expands every base row into both wrap variants and into every
//! execution mode the row declares, which for the curated rows includes the
//! reporting modes. The self-tests below pin that expansion, and
//! [`cli_matrix_snapshots`] records it through the real binary.

use std::collections::{BTreeMap, BTreeSet};

#[path = "cli_matrix/support.rs"]
mod support;

use support::{
    ALL_FLAGS,
    BASE_MATRIX_CASES,
    BaseCase,
    ExecutionMode,
    WrapVariant,
    assert_reporting_invariants,
    assert_transform_invariants,
    check_counts,
    diff_counts,
    fixture_has_table,
    fixture_path,
    has_flag,
    is_case_id,
    logical_cases,
    non_wrap_signature,
    physical_case,
    physical_cases,
    reporting_cases,
    run_physical_case,
};

#[test]
fn matrix_case_ids_are_unique() {
    let mut ids = BTreeSet::new();
    for case in BASE_MATRIX_CASES {
        assert!(is_case_id(case.id), "invalid case id {}", case.id);
        assert!(ids.insert(case.id), "duplicate case id {}", case.id);
    }
}

#[test]
fn matrix_case_ids_accept_documented_characters() {
    assert!(is_case_id("row-001_alpha2"));
}

#[test]
fn matrix_case_fixtures_are_dat_files() {
    for case in BASE_MATRIX_CASES {
        let fixture = fixture_path(case.fixture);
        assert!(fixture.exists(), "missing fixture {}", fixture.display());
        assert_eq!(
            fixture.extension().and_then(|ext| ext.to_str()),
            Some("dat"),
            "a reporting mode echoes the name the file was staged under, and the harness stages \
             every fixture as 'input.dat', so only '.dat' fixtures stay snapshot-stable",
        );
    }
}

/// Every logical case runs the two printing modes plus exactly the reporting
/// modes its base row declares: no more, and no fewer.
#[test]
fn matrix_cases_expand_to_their_declared_modes() {
    let mut modes_by_logical_id: BTreeMap<String, BTreeSet<ExecutionMode>> = BTreeMap::new();
    for case in physical_cases() {
        modes_by_logical_id
            .entry(case.logical.id)
            .or_default()
            .insert(case.mode);
    }

    for logical in logical_cases() {
        let mut expected = BTreeSet::from([ExecutionMode::Stdout, ExecutionMode::InPlace]);
        expected.extend(logical.reporting.iter().copied());
        assert_eq!(
            modes_by_logical_id.get(&logical.id),
            Some(&expected),
            "logical case {} must run exactly the modes it declares",
            logical.id,
        );
    }
}

#[test]
fn matrix_cases_expand_to_wrapped_and_unwrapped() {
    let mut wraps_by_signature: BTreeMap<String, BTreeSet<WrapVariant>> = BTreeMap::new();
    for case in logical_cases() {
        let variant = if case.is_wrapped {
            WrapVariant::Wrapped
        } else {
            WrapVariant::Unwrapped
        };
        wraps_by_signature
            .entry(non_wrap_signature(case.fixture, &case.flags))
            .or_default()
            .insert(variant);
    }

    for (signature, variants) in wraps_by_signature {
        assert_eq!(
            variants,
            BTreeSet::from([WrapVariant::Wrapped, WrapVariant::Unwrapped]),
            "non-wrap signature {signature} must have both wrap variants",
        );
    }
}

#[test]
fn matrix_cases_cover_all_transform_pairs() {
    for (left_index, left) in ALL_FLAGS.iter().enumerate() {
        for right in ALL_FLAGS.iter().skip(left_index + 1) {
            let mut combinations = BTreeSet::new();
            for case in BASE_MATRIX_CASES {
                combinations.insert((has_flag(case, *left), has_flag(case, *right)));
            }
            assert_eq!(
                combinations,
                BTreeSet::from([(false, false), (false, true), (true, false), (true, true)]),
                "missing pair coverage for {} and {}",
                left.as_arg(),
                right.as_arg(),
            );
        }
    }
}

#[test]
fn matrix_cases_enable_and_disable_each_transform() {
    for flag in ALL_FLAGS {
        let mut states = BTreeSet::new();
        for case in BASE_MATRIX_CASES {
            states.insert(has_flag(case, *flag));
        }
        assert_eq!(
            states,
            BTreeSet::from([false, true]),
            "{} must appear enabled and disabled",
            flag.as_arg(),
        );
    }
}

/// The reporting subset is curated on purpose: it declares only modes the
/// printing rows do not already run, no curated row is half-covered, and it
/// stays narrower than the matrix it is drawn from.
#[test]
fn matrix_reporting_rows_are_curated() -> anyhow::Result<()> {
    for case in BASE_MATRIX_CASES {
        let declared: BTreeSet<ExecutionMode> = case.reporting.iter().copied().collect();
        assert_eq!(
            declared.len(),
            case.reporting.len(),
            "{} declares a reporting mode twice",
            case.id,
        );
        for mode in case.reporting {
            assert!(
                mode.reports(),
                "{} declares {mode:?}, which every row already runs",
                case.id,
            );
        }
    }

    let rows: Vec<&BaseCase> = BASE_MATRIX_CASES
        .iter()
        .filter(|case| !case.reporting.is_empty())
        .collect();
    assert!(!rows.is_empty(), "the reporting modes would go untested");
    assert!(
        rows.len() < BASE_MATRIX_CASES.len(),
        "the reporting subset must stay curated rather than expand the whole matrix",
    );

    let mut fixtures = BTreeSet::new();
    let mut tables = 0;
    for row in rows {
        let declared: BTreeSet<ExecutionMode> = row.reporting.iter().copied().collect();
        assert_eq!(
            declared,
            BTreeSet::from([ExecutionMode::Check, ExecutionMode::Diff]),
            "{} must carry both reporting modes through both wrap variants",
            row.id,
        );
        fixtures.insert(row.fixture);
        if fixture_has_table(row.fixture)? {
            tables += 1;
        }
    }
    assert!(
        tables > 0,
        "the reporting modes must be exercised on a document that needs table repair, which is \
         the drift they exist to report",
    );
    assert!(
        fixtures.len() > 1,
        "the reporting subset must not rest on a single fixture",
    );

    Ok(())
}

/// The two reporting modes share one assessment, so they must report the same
/// edit: the counts `--check` prints are the lines `--diff` marks.
///
/// A disagreement means one of the modes worked drift out for itself rather
/// than reporting what the shared assessment found, which is exactly the
/// divergence the curated rows are here to catch.
#[test]
fn matrix_reporting_modes_agree() {
    for logical in reporting_cases() {
        let check_case = physical_case(&logical, ExecutionMode::Check);
        let diff_case = physical_case(&logical, ExecutionMode::Diff);
        let check = run_physical_case(&check_case).expect("run physical case");
        let diff = run_physical_case(&diff_case).expect("run physical case");

        let reported = check_counts(&check_case, &check);
        let marked = diff_counts(&diff_case, &diff);

        assert_eq!(
            marked,
            reported,
            "{}: --diff marks {} insertions and {} deletions, while --check reports {} and {}",
            logical.id,
            marked.insertions,
            marked.deletions,
            reported.insertions,
            reported.deletions,
        );
    }
}

#[test]
fn cli_matrix_snapshots() -> anyhow::Result<()> {
    let mut drifting = 0;
    let mut clean = 0;
    for logical in logical_cases() {
        let stdout_case = physical_case(&logical, ExecutionMode::Stdout);
        let in_place_case = physical_case(&logical, ExecutionMode::InPlace);

        let stdout_result = run_physical_case(&stdout_case).expect("run physical case");
        assert!(
            stdout_result.output.status.success(),
            "{} failed with stderr:\n{}",
            stdout_case.snapshot_name(),
            String::from_utf8_lossy(&stdout_result.output.stderr),
        );
        assert_transform_invariants(&stdout_case.logical, &stdout_result.output.stdout)?;

        let in_place_result = run_physical_case(&in_place_case).expect("run physical case");
        assert!(
            in_place_result.output.status.success(),
            "{} failed with stderr:\n{}",
            in_place_case.snapshot_name(),
            String::from_utf8_lossy(&in_place_result.output.stderr),
        );
        assert_transform_invariants(&in_place_case.logical, &in_place_result.file_content)?;

        assert_eq!(
            stdout_result.output.stdout, in_place_result.file_content,
            "{} stdout must match in-place file output",
            stdout_case.logical.id,
        );

        insta::assert_snapshot!(
            stdout_case.snapshot_name(),
            stdout_result.envelope(&stdout_case)
        );
        insta::assert_snapshot!(
            in_place_case.snapshot_name(),
            in_place_result.envelope(&in_place_case),
        );

        // The printed document is the oracle a report is measured against, so
        // it is computed once per logical case rather than re-derived.
        let printed = String::from_utf8_lossy(&stdout_result.output.stdout);
        for mode in &logical.reporting {
            let case = physical_case(&logical, *mode);
            let result = run_physical_case(&case).expect("run physical case");
            if assert_reporting_invariants(&case, &printed, &result)? {
                drifting += 1;
            } else {
                clean += 1;
            }
            insta::assert_snapshot!(case.snapshot_name(), result.envelope(&case));
        }
    }

    assert!(drifting > 0, "the curated rows must cover a drifting file");
    assert!(
        clean > 0,
        "a report that always fired would satisfy the drifting rows alone, so the curated rows \
         must also cover a file that needs no change",
    );

    Ok(())
}
