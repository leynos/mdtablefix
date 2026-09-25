//! Corpus integrity checks for the idempotence integration tests.

use std::fs;

use super::{CASES, corpus_dir, corpus_path};

#[test]
fn corpus_case_ids_and_fixtures_are_unique_and_present() {
    let mut fixtures = std::collections::BTreeSet::new();

    for case in CASES {
        assert!(
            fixtures.insert(case.fixture),
            "duplicate fixture {} in case {}",
            case.fixture,
            case.id,
        );
        let path = corpus_path(case.fixture);
        assert!(path.exists(), "missing fixture {}", path.display());
        assert_eq!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("dat"),
            "fixture {} must be a .dat file",
            case.fixture,
        );
    }

    let on_disk: std::collections::BTreeSet<String> = fs::read_dir(corpus_dir())
        .expect("reading the corpus directory")
        .map(|entry| {
            entry
                .expect("reading a corpus entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let recorded: std::collections::BTreeSet<String> =
        CASES.iter().map(|case| case.fixture.to_string()).collect();

    assert_eq!(
        on_disk, recorded,
        "every fixture on disk must be recorded in the case table",
    );
}

#[test]
fn corpus_records_a_flag_set_for_every_case() {
    for case in CASES {
        assert!(!case.flags.is_empty(), "case {} records no flags", case.id);
        assert!(
            case.flags.iter().all(|flag| flag.starts_with("--")),
            "case {} records a non-flag argument",
            case.id,
        );
    }
}
