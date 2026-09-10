//! Metrics tests for the atomic replacement path.
//!
//! A recorder installed with `metrics::with_local_recorder` captures what the
//! replacement emits on this thread, so the tests assert both the names a host
//! application would aggregate and the bounded label set it would have to
//! store. The library installs no recorder of its own.

use std::fs;

use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};
use tempfile::tempdir;

use super::{TEMP_FILE_ATTEMPTS, rewrite, temporary_path};

/// The outcome label's name.
const OUTCOME_LABEL: &str = "outcome";

/// The only values the outcome label may take.
const OUTCOMES: [&str; 2] = ["success", "failure"];

/// A recorded metric: its name, its labels, and its value.
type Recorded = (String, Vec<(String, String)>, u64);

/// Records the metrics emitted on this thread while `f` runs.
fn recorded<T>(f: impl FnOnce() -> T) -> (T, Vec<Recorded>) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let value = metrics::with_local_recorder(&recorder, f);
    (value, counters(snapshotter.snapshot()))
}

/// Flattens a snapshot into the counter triples the assertions read.
fn counters(snapshot: Snapshot) -> Vec<Recorded> {
    snapshot
        .into_vec()
        .into_iter()
        .map(|(composite, _unit, _description, value)| {
            let key = composite.key();
            let labels = key
                .labels()
                .map(|label| (label.key().to_string(), label.value().to_string()))
                .collect();
            match value {
                DebugValue::Counter(count) => (key.name().to_string(), labels, count),
                other => panic!("the replacement path must emit counters, found {other:?}"),
            }
        })
        .collect()
}

/// Returns the count recorded for `name` under exactly `labels`, or zero when
/// nothing was recorded.
fn count(recorded: &[Recorded], name: &str, labels: &[(&str, &str)]) -> u64 {
    recorded
        .iter()
        .find(|(recorded_name, recorded_labels, _)| {
            recorded_name == name && same_labels(recorded_labels, labels)
        })
        .map_or(0, |(_, _, value)| *value)
}

/// Returns the count recorded for a replacement that ended as `outcome`.
fn outcome_count(recorded: &[Recorded], outcome: &str) -> u64 {
    count(
        recorded,
        "mdtablefix_io_replace_total",
        &[(OUTCOME_LABEL, outcome)],
    )
}

/// Reports whether a recorded label set carries exactly `expected`.
fn same_labels(recorded: &[(String, String)], expected: &[(&str, &str)]) -> bool {
    recorded.len() == expected.len()
        && expected.iter().all(|(key, value)| {
            recorded.iter().any(|(recorded_key, recorded_value)| {
                recorded_key == key && recorded_value == value
            })
        })
}

/// Asserts that the label space is bounded: the only label key is `outcome`,
/// and its values are drawn from the fixed set above.
fn assert_labels_are_bounded(recorded: &[Recorded]) {
    for (name, labels, _) in recorded {
        for (key, value) in labels {
            assert_eq!(
                *key, OUTCOME_LABEL,
                "{name} must not label metrics with {key}"
            );
            assert!(
                OUTCOMES.contains(&value.as_str()),
                "{name} must stay inside the declared outcome set: {value}"
            );
        }
    }
}

/// Writes a fixture that needs reflowing and returns its path.
fn fixture(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let file = dir.path().join("sample.md");
    fs::write(&file, "|A|B|\n|1|2|").expect("write fixture");
    file
}

#[test]
fn successful_replacement_is_counted() {
    let dir = tempdir().expect("create temporary directory");
    let file = fixture(&dir);

    let (result, recorded) = recorded(|| rewrite(&file));

    result.expect("the rewrite must succeed");
    assert_labels_are_bounded(&recorded);
    assert_eq!(
        outcome_count(&recorded, "success"),
        1,
        "a completed replacement is a success: {recorded:?}"
    );
    assert_eq!(
        count(
            &recorded,
            "mdtablefix_io_temporary_name_collisions_total",
            &[]
        ),
        0,
        "an uncontended name space records no collision: {recorded:?}"
    );
    assert_eq!(
        count(
            &recorded,
            "mdtablefix_io_temporary_name_exhausted_total",
            &[]
        ),
        0,
        "a free candidate name records no exhaustion: {recorded:?}"
    );
}

#[test]
fn an_occupied_candidate_is_counted_as_a_collision() {
    let dir = tempdir().expect("create temporary directory");
    let file = fixture(&dir);
    // Candidate names are a pure function of the target, the process id, and
    // the attempt, so the test can occupy the first candidate exactly.
    let occupied = temporary_path(camino::Utf8Path::new("sample.md"), 0);
    fs::write(
        dir.path()
            .join(occupied.file_name().expect("candidate name")),
        "",
    )
    .expect("occupy the first candidate name");

    let (result, recorded) = recorded(|| rewrite(&file));

    result.expect("the rewrite must retry past the occupied name");
    assert_labels_are_bounded(&recorded);
    assert_eq!(
        count(
            &recorded,
            "mdtablefix_io_temporary_name_collisions_total",
            &[]
        ),
        1,
        "one occupied candidate is one collision: {recorded:?}"
    );
    assert_eq!(
        outcome_count(&recorded, "success"),
        1,
        "a retried replacement still succeeds: {recorded:?}"
    );
}

#[test]
fn an_exhausted_name_space_is_counted() {
    let dir = tempdir().expect("create temporary directory");
    let file = fixture(&dir);
    for attempt in 0..TEMP_FILE_ATTEMPTS {
        let candidate = temporary_path(camino::Utf8Path::new("sample.md"), attempt);
        fs::write(
            dir.path()
                .join(candidate.file_name().expect("candidate name")),
            "",
        )
        .expect("occupy the candidate name");
    }

    let (result, recorded) = recorded(|| rewrite(&file));

    let error = result.expect_err("every candidate name is occupied");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_labels_are_bounded(&recorded);
    assert_eq!(
        count(
            &recorded,
            "mdtablefix_io_temporary_name_collisions_total",
            &[]
        ),
        u64::from(TEMP_FILE_ATTEMPTS),
        "every occupied candidate is a collision: {recorded:?}"
    );
    assert_eq!(
        count(
            &recorded,
            "mdtablefix_io_temporary_name_exhausted_total",
            &[]
        ),
        1,
        "exhausting the name space is counted once: {recorded:?}"
    );
    assert_eq!(
        outcome_count(&recorded, "failure"),
        1,
        "an abandoned replacement is a failure: {recorded:?}"
    );
}

/// The replacement path emits the metrics its own documentation and the host
/// application's dashboards name, so a rename here must be deliberate.
#[test]
fn emitted_metric_names_are_stable() {
    let dir = tempdir().expect("create temporary directory");
    let file = fixture(&dir);

    let ((), recorded) = recorded(|| {
        rewrite(&file).expect("the rewrite must succeed");
    });

    let names: Vec<&str> = recorded.iter().map(|(name, ..)| name.as_str()).collect();
    assert_eq!(
        names,
        vec!["mdtablefix_io_replace_total"],
        "only the outcome counter is expected for an uncontended replacement"
    );
}
