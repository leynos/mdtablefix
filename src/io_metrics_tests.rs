//! Metrics tests for the atomic replacement path.
//!
//! A recorder installed with `metrics::with_local_recorder` captures what the
//! replacement emits on this thread, so the tests assert both the names a host
//! application would aggregate and the bounded label set it would have to
//! store. The library installs no recorder of its own.

use std::fs;

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use metrics::Unit;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};
use tempfile::tempdir;

use super::{
    TEMP_FILE_ATTEMPTS,
    register_metrics,
    remove_failed_temporary_file,
    rewrite,
    temporary_path,
};

/// The outcome label's name.
const OUTCOME_LABEL: &str = "outcome";

/// The only values the outcome label may take.
const OUTCOMES: [&str; 2] = ["success", "failure"];

/// The counter recording replacement outcomes.
const REPLACE_TOTAL: &str = "mdtablefix_io_replace_total";

/// The histogram recording replacement durations.
const REPLACE_DURATION: &str = "mdtablefix_io_replace_duration_seconds";

/// The counter recording temporary names that were already taken.
const COLLISIONS: &str = "mdtablefix_io_temporary_name_collisions_total";

/// The counter recording exhausted temporary name spaces.
const EXHAUSTED: &str = "mdtablefix_io_temporary_name_exhausted_total";

/// The counter recording temporary files a failed replacement could not
/// remove.
const CLEANUP_FAILURES: &str = "mdtablefix_io_temporary_cleanup_failures_total";

/// What a recorded metric carried.
#[derive(Debug)]
enum Value {
    /// A counter's value.
    Counter(u64),
    /// A histogram's samples.
    Histogram(Vec<f64>),
}

/// A recorded metric: its name, its labels, its declared unit and description,
/// and its value.
#[derive(Debug)]
struct Recorded {
    /// The metric name a recorder would aggregate under.
    name: String,
    /// The labels it was recorded with.
    labels: Vec<(String, String)>,
    /// The unit declared by `register_metrics`, if any.
    unit: Option<Unit>,
    /// The description declared by `register_metrics`, if any.
    description: Option<String>,
    /// The recorded value.
    value: Value,
}

/// Records the metrics emitted on this thread while `f` runs.
///
/// The descriptions are registered with the test's own recorder first. The
/// replacement registers them once per process, so without this the assertions
/// on the declared unit and description would depend on which test happened to
/// warm that lock.
fn recorded<T>(f: impl FnOnce() -> T) -> (T, Vec<Recorded>) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let value = metrics::with_local_recorder(&recorder, || {
        register_metrics();
        f()
    });
    (value, metrics(snapshotter.snapshot()))
}

/// Flattens a snapshot into the metrics the assertions read.
fn metrics(snapshot: Snapshot) -> Vec<Recorded> {
    snapshot
        .into_vec()
        .into_iter()
        .map(|(composite, unit, description, value)| {
            let key = composite.key();
            let labels = key
                .labels()
                .map(|label| (label.key().to_string(), label.value().to_string()))
                .collect();
            let name = key.name().to_string();
            let value = match value {
                DebugValue::Counter(count) => Value::Counter(count),
                DebugValue::Histogram(samples) => {
                    Value::Histogram(samples.iter().map(|sample| sample.0).collect())
                }
                DebugValue::Gauge(gauge) => {
                    panic!("the replacement path must not emit a gauge: {gauge:?}")
                }
            };
            Recorded {
                name,
                labels,
                unit,
                description: description.map(|text| text.to_string()),
                value,
            }
        })
        .collect()
}

/// Returns the recorded metric named `name` carrying exactly `labels`.
fn find<'a>(recorded: &'a [Recorded], name: &str, labels: &[(&str, &str)]) -> Option<&'a Recorded> {
    recorded
        .iter()
        .find(|metric| metric.name == name && same_labels(&metric.labels, labels))
}

/// Returns the count recorded for `name` under exactly `labels`, or zero when
/// nothing was recorded.
fn count(recorded: &[Recorded], name: &str, labels: &[(&str, &str)]) -> u64 {
    match find(recorded, name, labels) {
        Some(Recorded {
            value: Value::Counter(count),
            ..
        }) => *count,
        Some(other) => panic!("{name} must be a counter, found {other:?}"),
        None => 0,
    }
}

/// Returns the samples recorded for the histogram `name` under exactly
/// `labels`, or an empty slice when nothing was recorded.
fn samples<'a>(recorded: &'a [Recorded], name: &str, labels: &[(&str, &str)]) -> &'a [f64] {
    match find(recorded, name, labels) {
        Some(Recorded {
            value: Value::Histogram(samples),
            ..
        }) => samples,
        Some(other) => panic!("{name} must be a histogram, found {other:?}"),
        None => &[],
    }
}

/// Reports whether the metric `name`, under exactly `labels`, carries a
/// non-empty description.
fn is_described(recorded: &[Recorded], name: &str, labels: &[(&str, &str)]) -> bool {
    find(recorded, name, labels)
        .and_then(|metric| metric.description.as_deref())
        .is_some_and(|text| !text.is_empty())
}

/// Returns the unit declared for `name`, if the metric was recorded at all.
fn unit(recorded: &[Recorded], name: &str) -> Option<Unit> {
    recorded
        .iter()
        .find(|metric| metric.name == name)
        .and_then(|metric| metric.unit)
}

/// Returns the count recorded for a replacement that ended as `outcome`.
fn outcome_count(recorded: &[Recorded], outcome: &str) -> u64 {
    count(recorded, REPLACE_TOTAL, &[(OUTCOME_LABEL, outcome)])
}

/// Returns the durations recorded for a replacement that ended as `outcome`.
fn outcome_samples(recorded: &[Recorded], outcome: &str) -> Vec<f64> {
    samples(recorded, REPLACE_DURATION, &[(OUTCOME_LABEL, outcome)]).to_vec()
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
    for metric in recorded {
        for (key, value) in &metric.labels {
            assert_eq!(
                *key, OUTCOME_LABEL,
                "{} must not label metrics with {key}",
                metric.name
            );
            assert!(
                OUTCOMES.contains(&value.as_str()),
                "{} must stay inside the declared outcome set: {value}",
                metric.name
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
        count(&recorded, COLLISIONS, &[]),
        0,
        "an uncontended name space records no collision: {recorded:?}"
    );
    assert_eq!(
        count(&recorded, EXHAUSTED, &[]),
        0,
        "a free candidate name records no exhaustion: {recorded:?}"
    );
}

/// A replacement is timed for every outcome, and the unit is declared, so a
/// dashboard reading the histogram knows what the samples are measured in.
#[test]
fn replacement_duration_is_recorded() {
    let dir = tempdir().expect("create temporary directory");
    let file = fixture(&dir);

    let (result, recorded) = recorded(|| rewrite(&file));

    result.expect("the rewrite must succeed");
    assert_labels_are_bounded(&recorded);
    assert_eq!(
        unit(&recorded, REPLACE_DURATION),
        Some(Unit::Seconds),
        "the duration must be declared in seconds: {recorded:?}"
    );
    assert!(
        is_described(&recorded, REPLACE_DURATION, &[(OUTCOME_LABEL, "success")]),
        "the histogram must carry a description: {recorded:?}"
    );
    let durations = outcome_samples(&recorded, "success");
    assert_eq!(
        durations.len(),
        1,
        "a replacement records exactly one duration: {recorded:?}"
    );
    assert!(
        durations[0] > 0.0,
        "a replacement takes measurable time: {durations:?}"
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
        count(&recorded, COLLISIONS, &[]),
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
        count(&recorded, COLLISIONS, &[]),
        u64::from(TEMP_FILE_ATTEMPTS),
        "every occupied candidate is a collision: {recorded:?}"
    );
    assert_eq!(
        count(&recorded, EXHAUSTED, &[]),
        1,
        "exhausting the name space is counted once: {recorded:?}"
    );
    assert_eq!(
        outcome_count(&recorded, "failure"),
        1,
        "an abandoned replacement is a failure: {recorded:?}"
    );
    assert_eq!(
        outcome_samples(&recorded, "failure").len(),
        1,
        "a failed replacement is timed too, so stalls before failure are visible: {recorded:?}"
    );
    assert_eq!(
        count(&recorded, CLEANUP_FAILURES, &[]),
        0,
        "a replacement abandoned before its temporary file existed has none to clean up: \
         {recorded:?}"
    );
}

/// Unix-only tests, with the constants they use, kept together so that the
/// whole group is compiled out together on other targets: a symbol left at
/// module level would be dead code, and therefore a denied warning, wherever
/// its only test is removed.
#[cfg(unix)]
mod unix {
    use super::*;

    /// The counter recording symbolic-link targets declined rather than
    /// replaced.
    const SYMLINK_DECLINED: &str = "mdtablefix_io_symlink_declined_total";

    /// A declined symbolic link is counted as its own event as well as a
    /// failure, so an operator can tell "the target is a symlink" from "the
    /// replacement failed" without reading the log.
    #[test]
    fn a_declined_symlink_is_counted() {
        let dir = tempdir().expect("create temporary directory");
        fixture(&dir);
        let link = dir.path().join("link.md");
        // A relative target keeps the link resolvable inside the capability.
        std::os::unix::fs::symlink("sample.md", &link).expect("create the symlink");

        let (result, recorded) = recorded(|| rewrite(&link));

        let error = result.expect_err("a symlink target must be declined");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert_labels_are_bounded(&recorded);
        assert_eq!(
            count(&recorded, SYMLINK_DECLINED, &[]),
            1,
            "a declined symlink is counted once: {recorded:?}"
        );
        assert!(
            is_described(&recorded, SYMLINK_DECLINED, &[]),
            "the counter must carry a description: {recorded:?}"
        );
        assert_eq!(
            outcome_count(&recorded, "failure"),
            1,
            "a declined symlink is a replacement that did not happen: {recorded:?}"
        );
        assert_eq!(
            count(&recorded, CLEANUP_FAILURES, &[]),
            0,
            "a declined symlink never created a temporary file: {recorded:?}"
        );
    }
}

/// A cleanup that does not complete is counted, because the failure that
/// prompted it is the one the caller sees: the counter is the only signal that
/// a stale temporary file was left beside the target.
#[test]
fn a_cleanup_that_cannot_remove_the_temporary_file_is_counted() {
    let dir = tempdir().expect("create temporary directory");
    let root = Utf8Path::from_path(dir.path()).expect("the temporary directory is UTF-8");
    let directory =
        Dir::open_ambient_dir(root, ambient_authority()).expect("open the directory capability");
    // `std::fs::remove_file` is documented to fail when the path points to a
    // directory, on every platform, so the cleanup fails without the test
    // depending on a permission bit, which root would ignore.
    fs::create_dir(dir.path().join("taken.tmp")).expect("create the unremovable entry");

    let ((), recorded) =
        recorded(|| remove_failed_temporary_file(&directory, Utf8Path::new("taken.tmp")));

    assert_labels_are_bounded(&recorded);
    assert_eq!(
        count(&recorded, CLEANUP_FAILURES, &[]),
        1,
        "a cleanup that did not complete is counted once: {recorded:?}"
    );
    assert!(
        is_described(&recorded, CLEANUP_FAILURES, &[]),
        "the counter must carry a description: {recorded:?}"
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

    let mut names: Vec<&str> = recorded.iter().map(|metric| metric.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec![REPLACE_DURATION, REPLACE_TOTAL],
        "only the outcome metrics are expected for an uncontended replacement"
    );
}
