//! Matrix tests for CLI option interactions.

use std::collections::{BTreeMap, BTreeSet};

#[path = "cli_matrix/support.rs"]
mod support;

use support::{
    ALL_FLAGS,
    BASE_MATRIX_CASES,
    ExecutionMode,
    PhysicalCase,
    WrapVariant,
    assert_transform_invariants,
    fixture_path,
    has_flag,
    is_case_id,
    logical_cases,
    non_wrap_signature,
    physical_cases,
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
            Some("dat")
        );
    }
}

#[test]
fn matrix_cases_expand_to_stdout_and_in_place() {
    let mut modes_by_logical_id: BTreeMap<String, BTreeSet<ExecutionMode>> = BTreeMap::new();
    for case in physical_cases() {
        modes_by_logical_id
            .entry(case.logical.id)
            .or_default()
            .insert(case.mode);
    }

    for (id, modes) in modes_by_logical_id {
        assert_eq!(
            modes,
            BTreeSet::from([ExecutionMode::Stdout, ExecutionMode::InPlace]),
            "logical case {id} must run in both modes",
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

#[test]
fn cli_matrix_snapshots() -> anyhow::Result<()> {
    for logical in logical_cases() {
        let stdout_case = PhysicalCase {
            logical: logical.clone(),
            mode: ExecutionMode::Stdout,
        };
        let in_place_case = PhysicalCase {
            logical,
            mode: ExecutionMode::InPlace,
        };

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
    }
    Ok(())
}
