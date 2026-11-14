//! Metrics tests for the wrapping boundary.
//!
//! A recorder installed with `metrics::with_local_recorder` captures what
//! [`wrap_text`](super::wrap_text) emits on this thread, so the tests assert
//! the names a host would aggregate and the bounded label set it would have to
//! store.

use metrics::Unit;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};

use super::{
    metrics::{REFLOWED, UNCHANGED, WRAP_DURATION, WRAP_TOTAL},
    wrap_text,
};

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
/// Nothing is registered up front: `f` must drive the boundary for itself, so a
/// missing or mis-timed description registration shows up as a failed
/// assertion rather than being masked by the test.
fn recorded<T>(f: impl FnOnce() -> T) -> (T, Vec<Recorded>) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let value = metrics::with_local_recorder(&recorder, f);
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
                        Value::Histogram(samples.iter().map(|sample| sample.into_inner()).collect())
                    }
                    DebugValue::Gauge(gauge) => panic!("unexpected gauge metric: {gauge:?}"),
                },
            }
        })
        .collect()
}

/// Returns the one recorded metric named `name`.
///
/// A miss is the test's verdict rather than an arrangement failure: these
/// helpers read metrics the test has just recorded, so an absent or duplicated
/// entry means the expectation under test did not hold. The `let`-`else` states
/// that outcome directly; `unwrap_or_else` would spell a failure as a default
/// value the caller never receives.
fn only<'a>(recorded: &'a [Recorded], name: &str) -> &'a Recorded {
    let mut matching = recorded.iter().filter(|metric| metric.name == name);
    let Some(metric) = matching.next() else {
        panic!("no metric named {name}");
    };
    assert!(
        matching.next().is_none(),
        "more than one metric named {name}"
    );
    metric
}

/// Returns the recorded metric named `name` whose `outcome` label is `outcome`.
fn with_outcome<'a>(recorded: &'a [Recorded], name: &str, outcome: &str) -> &'a Recorded {
    let found = recorded.iter().find(|metric| {
        metric.name == name
            && metric
                .labels
                .iter()
                .any(|(key, value)| key == "outcome" && value == outcome)
    });
    let Some(metric) = found else {
        panic!("no {name} carrying outcome={outcome}");
    };
    metric
}

/// Returns the `outcome` label of a recorded metric.
fn outcome(metric: &Recorded) -> &str {
    let found = metric.labels.iter().find(|(key, _)| key == "outcome");
    let Some((_, value)) = found else {
        panic!("no outcome label on {}", metric.name);
    };
    value
}

#[test]
fn wrap_text_counts_a_reflowed_invocation() {
    let input = vec!["a very long line that must be reflowed at a narrow width".to_string()];

    let (output, recorded) = recorded(|| wrap_text(&input, 20));

    assert_ne!(output, input);
    let counter = only(&recorded, WRAP_TOTAL);
    assert_eq!(outcome(counter), REFLOWED);
    assert!(matches!(counter.value, Value::Counter(1)));
}

#[test]
fn wrap_text_counts_an_unchanged_invocation() {
    let input = vec!["short".to_string()];

    let (output, recorded) = recorded(|| wrap_text(&input, 80));

    assert_eq!(output, input);
    let counter = only(&recorded, WRAP_TOTAL);
    assert_eq!(outcome(counter), UNCHANGED);
    assert!(matches!(counter.value, Value::Counter(1)));
}

/// A host may wrap before it installs a recorder. The descriptions emitted by
/// that first wrap reach the no-op recorder, so they must be re-emitted for
/// whichever recorder is active later; a process-wide cache would leave the
/// later histogram without its unit or description.
#[test]
fn a_recorder_installed_after_a_wrap_still_receives_the_descriptions() {
    let long = vec!["a very long line that must be reflowed at a narrow width".to_string()];
    let short = vec!["short".to_string()];

    // No recorder is active here, so these descriptions have nowhere to go.
    assert_ne!(wrap_text(&long, 20), long);

    let (outputs, recorded) = recorded(|| (wrap_text(&long, 20), wrap_text(&short, 80)));
    assert_ne!(outputs.0, long, "the long line must be reflowed");
    assert_eq!(outputs.1, short, "the short line must be left alone");

    let mut names: Vec<&str> = recorded.iter().map(|metric| metric.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names,
        vec!["mdtablefix_wrap_duration_seconds", "mdtablefix_wrap_total"],
        "the boundary must report only its two documented names: {recorded:?}"
    );

    for metric in &recorded {
        assert!(
            metric
                .description
                .as_deref()
                .is_some_and(|text| !text.is_empty()),
            "{} arrived without a description: {metric:?}",
            metric.name
        );
        assert_eq!(
            metric.labels.len(),
            1,
            "{} must carry only the bounded outcome label: {metric:?}",
            metric.name
        );
    }

    for expected in [REFLOWED, UNCHANGED] {
        let counter = with_outcome(&recorded, WRAP_TOTAL, expected);
        assert!(
            matches!(counter.value, Value::Counter(1)),
            "{expected} is counted once: {counter:?}"
        );
        let histogram = with_outcome(&recorded, WRAP_DURATION, expected);
        assert_eq!(
            histogram.unit,
            Some(Unit::Seconds),
            "the duration must be declared in seconds: {histogram:?}"
        );
        match &histogram.value {
            Value::Histogram(samples) => assert_eq!(
                samples.len(),
                1,
                "{expected} records exactly one duration sample: {histogram:?}"
            ),
            Value::Counter(count) => panic!("expected a histogram, got counter {count}"),
        }
    }
}

#[test]
fn wrap_text_records_one_duration_sample_per_invocation() {
    let input = vec!["short".to_string()];

    let (_, recorded) = recorded(|| wrap_text(&input, 80));

    let histogram = only(&recorded, WRAP_DURATION);
    assert_eq!(histogram.unit, Some(Unit::Seconds));
    assert!(histogram.description.is_some());
    match &histogram.value {
        Value::Histogram(samples) => assert_eq!(samples.len(), 1),
        Value::Counter(count) => panic!("expected a histogram, got counter {count}"),
    }
}
