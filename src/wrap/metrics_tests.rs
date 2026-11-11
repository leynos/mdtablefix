//! Metrics tests for the wrapping boundary.
//!
//! A recorder installed with `metrics::with_local_recorder` captures what
//! [`wrap_text`](super::wrap_text) emits on this thread, so the tests assert
//! the names a host would aggregate and the bounded label set it would have to
//! store.

use metrics::Unit;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};

use super::{
    metrics::{REFLOWED, UNCHANGED, WRAP_DURATION, WRAP_TOTAL, register_metrics},
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
/// The descriptions are registered with the test's own recorder first, so the
/// assertions on the declared unit and description do not depend on which test
/// warmed the boundary's one-per-process lock.
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
fn only<'a>(recorded: &'a [Recorded], name: &str) -> &'a Recorded {
    let mut matching = recorded.iter().filter(|metric| metric.name == name);
    let metric = matching
        .next()
        .unwrap_or_else(|| panic!("no metric named {name}"));
    assert!(
        matching.next().is_none(),
        "more than one metric named {name}"
    );
    metric
}

/// Returns the `outcome` label of a recorded metric.
fn outcome(metric: &Recorded) -> &str {
    metric
        .labels
        .iter()
        .find(|(key, _)| key == "outcome")
        .map_or_else(
            || panic!("no outcome label on {}", metric.name),
            |(_, value)| value,
        )
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
