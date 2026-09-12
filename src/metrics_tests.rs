//! Metrics tests for the binary's boundaries.
//!
//! A recorder installed with `metrics::with_local_recorder` captures what the
//! boundaries emit on this thread, so the tests assert the names a host would
//! aggregate and the bounded label set it would have to store. The harness is
//! shared with the file boundary's own tests in `metrics_file_tests`.

use std::{io, time::Duration};

use anyhow::Error;
use metrics::Unit;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};

use super::{FileOutcome, record_file, record_run, register_metrics};
use crate::driver::{ExitStatus, Mode};

const MODE_LABEL: &str = "mode";
const OUTCOME_LABEL: &str = "outcome";
const CATEGORY_LABEL: &str = "category";

/// The counter recording runs.
const RUN_TOTAL: &str = "mdtablefix_run_total";

/// The counter recording files analysed.
const FILE_TOTAL: &str = "mdtablefix_file_total";

/// The histogram recording each file's analysis duration.
const FILE_DURATION: &str = "mdtablefix_file_duration_seconds";

/// The counter recording files that could not be analysed.
const FILE_ERROR_TOTAL: &str = "mdtablefix_file_error_total";

/// The only values the `mode` label may take.
const MODES: [&str; 4] = ["print", "in_place", "check", "diff"];

/// The only values the file outcome label may take.
const FILE_OUTCOMES: [&str; 3] = ["changed", "unchanged", "error"];

/// The only values the run outcome label may take.
const RUN_OUTCOMES: [&str; 3] = ["success", "drift", "error"];

/// The only values the error category label may take.
const CATEGORIES: [&str; 4] = ["not_found", "permission_denied", "declined", "other"];

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
pub(super) struct Recorded {
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
/// The descriptions are registered with the test's own recorder first, so the
/// assertions on the declared unit and description do not depend on which test
/// warmed the boundary's one-per-process lock.
pub(super) fn recorded<T>(f: impl FnOnce() -> T) -> (T, Vec<Recorded>) {
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
            Recorded {
                name: key.name().to_string(),
                labels,
                unit,
                description: description.map(|text| text.to_string()),
                value: match value {
                    DebugValue::Counter(count) => Value::Counter(count),
                    DebugValue::Histogram(samples) => {
                        Value::Histogram(samples.iter().map(|sample| sample.0).collect())
                    }
                    DebugValue::Gauge(gauge) => {
                        panic!("boundaries must not emit a gauge: {gauge:?}")
                    }
                },
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

/// Returns the count recorded for `name` under exactly `labels`, or zero.
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

/// Returns the histogram samples recorded for `name` under exactly `labels`.
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

/// Reports whether a recorded label set carries exactly `expected`.
fn same_labels(recorded: &[(String, String)], expected: &[(&str, &str)]) -> bool {
    recorded.len() == expected.len()
        && expected.iter().all(|(key, value)| {
            recorded.iter().any(|(recorded_key, recorded_value)| {
                recorded_key == key && recorded_value == value
            })
        })
}

/// The `mode` and `outcome` labels a run or file metric is recorded under.
fn outcome_labels<'a>(mode: &'a str, outcome: &'a str) -> [(&'a str, &'a str); 2] {
    [(MODE_LABEL, mode), (OUTCOME_LABEL, outcome)]
}

/// The count recorded for runs in `mode` that ended as `outcome`.
fn count_run(recorded: &[Recorded], mode: &str, outcome: &str) -> u64 {
    count(recorded, RUN_TOTAL, &outcome_labels(mode, outcome))
}

/// The count recorded for files analysed in `mode` that ended as `outcome`.
pub(super) fn count_file(recorded: &[Recorded], mode: &str, outcome: &str) -> u64 {
    count(recorded, FILE_TOTAL, &outcome_labels(mode, outcome))
}

/// The durations recorded for files analysed in `mode` that ended as `outcome`.
pub(super) fn durations<'a>(recorded: &'a [Recorded], mode: &str, outcome: &str) -> &'a [f64] {
    samples(recorded, FILE_DURATION, &outcome_labels(mode, outcome))
}

/// The count recorded for files that failed in `category`.
pub(super) fn count_error(recorded: &[Recorded], category: &str) -> u64 {
    count(recorded, FILE_ERROR_TOTAL, &[(CATEGORY_LABEL, category)])
}

/// Asserts that every recorded label is bounded by the declared sets.
///
/// A label value taken from a path, a file name, or an error message would grow
/// the series set with the tree the run was given. The sets are keyed by metric
/// as well as by label, because `outcome` is the run's status on one metric and
/// the file's fate on another: a run may exit `drift`, while a file may only be
/// `changed`.
pub(super) fn assert_labels_are_bounded(recorded: &[Recorded]) {
    for metric in recorded {
        let allowed = |key: &str| -> &'static [&'static str] {
            match (metric.name.as_str(), key) {
                (RUN_TOTAL | FILE_TOTAL | FILE_DURATION | FILE_ERROR_TOTAL, MODE_LABEL) => &MODES,
                (RUN_TOTAL, OUTCOME_LABEL) => &RUN_OUTCOMES,
                (FILE_TOTAL | FILE_DURATION, OUTCOME_LABEL) => &FILE_OUTCOMES,
                (FILE_ERROR_TOTAL, CATEGORY_LABEL) => &CATEGORIES,
                (_, other) => panic!("{} must not label metrics with {other}", metric.name),
            }
        };
        for (key, value) in &metric.labels {
            assert!(
                allowed(key).contains(&value.as_str()),
                "{} recorded {key}={value}, which is outside the declared set",
                metric.name
            );
        }
    }
}

/// A run is counted once, under the mode it ran in and the status it exited
/// with.
#[test]
fn a_run_is_counted_by_mode_and_outcome() {
    let ((), recorded) = recorded(|| {
        for mode in [Mode::Print, Mode::InPlace, Mode::Check, Mode::Diff] {
            for status in [ExitStatus::Success, ExitStatus::Drift, ExitStatus::Error] {
                record_run(mode, status);
            }
        }
    });

    for mode in MODES {
        for outcome in RUN_OUTCOMES {
            assert_eq!(
                count_run(&recorded, mode, outcome),
                1,
                "one run per mode and status must be counted: {mode}/{outcome}"
            );
        }
    }
    assert_labels_are_bounded(&recorded);
}

/// The status and mode labels are the declared sets, not `Debug`'s rendering of
/// the enums, which a variant rename would change.
#[test]
fn run_labels_take_the_declared_names() {
    let ((), recorded) = recorded(|| {
        record_run(Mode::InPlace, ExitStatus::Drift);
        record_run(Mode::Diff, ExitStatus::Error);
    });

    assert_eq!(count_run(&recorded, "in_place", "drift"), 1);
    assert_eq!(count_run(&recorded, "diff", "error"), 1);
    assert_eq!(
        count_run(&recorded, "InPlace", "Drift"),
        0,
        "a label is a fixed name rather than the variant's own spelling"
    );
}

/// Every metric the boundaries emit is declared, with a description and — for
/// the duration — a unit.
#[test]
fn every_metric_is_described() {
    let ((), recorded) = recorded(|| {
        record_run(Mode::Check, ExitStatus::Drift);
        record_file(Mode::Check, &FileOutcome::Changed, Duration::from_millis(1));
        record_file(
            Mode::Check,
            &FileOutcome::Failed(&Error::new(io::Error::new(
                io::ErrorKind::NotFound,
                "fixture",
            ))),
            Duration::from_millis(1),
        );
    });

    let mut names: Vec<&str> = recorded.iter().map(|metric| metric.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names,
        [FILE_DURATION, FILE_ERROR_TOTAL, FILE_TOTAL, RUN_TOTAL],
        "the four boundaries emit exactly the declared metrics"
    );
    for metric in &recorded {
        assert!(
            metric
                .description
                .as_deref()
                .is_some_and(|text| !text.is_empty()),
            "{} must declare a description, found {:?}",
            metric.name,
            metric.description
        );
    }
    let duration = recorded
        .iter()
        .find(|metric| metric.name == FILE_DURATION)
        .expect("the duration histogram must be recorded");
    assert_eq!(
        duration.unit,
        Some(Unit::Seconds),
        "the duration histogram must declare its unit"
    );
}
