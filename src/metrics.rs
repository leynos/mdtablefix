//! The metrics the binary's boundaries emit.
//!
//! The binary is a separate crate from the library, so it cannot share the
//! declarations in `mdtablefix::io`; it declares its own under the same
//! convention. Every name and label value is fixed, so a recorder's cardinality
//! stays bounded: no path, file name, or error text is ever a label. The binary
//! installs no recorder, exactly as the library installs none — a host that
//! wants these numbers installs one — and the tests install a local recorder to
//! assert the names, labels, and declared descriptions.

use std::{
    io,
    sync::OnceLock,
    time::{Duration, Instant},
};

use anyhow::Error;
use camino::Utf8Path;
use mdtablefix::report::FileReport;
use metrics::{Unit, counter, describe_counter, describe_histogram, histogram};
use tracing::{Span, debug, field};

use crate::driver::{ExitStatus, Mode};

/// Declares the descriptions of every metric the binary emits.
///
/// Called once, before the first run, so the names, units, and descriptions a
/// host would collect are registered even for a run that analyses nothing.
pub fn describe_metrics() {
    static DESCRIPTIONS: OnceLock<()> = OnceLock::new();
    DESCRIPTIONS.get_or_init(register_metrics);
}

/// Registers the descriptions of every metric the binary emits.
///
/// Split from [`describe_metrics`] so the tests can call it directly and assert
/// the declared unit and description without depending on which test warmed the
/// `OnceLock`, as the library's metrics seam does.
pub(super) fn register_metrics() {
    describe_counter!(
        "mdtablefix_run_total",
        "Runs of the tool, by mode and outcome"
    );
    describe_counter!(
        "mdtablefix_file_total",
        "Files analysed by a run, by mode and outcome"
    );
    describe_histogram!(
        "mdtablefix_file_duration_seconds",
        Unit::Seconds,
        "Duration of one file's analysis, by mode and outcome"
    );
    describe_counter!(
        "mdtablefix_file_error_total",
        "Files that could not be analysed, by error category"
    );
}

/// What became of one file's analysis.
pub enum FileOutcome<'a> {
    /// The formatter would write different bytes.
    Changed,
    /// The file is already formatted.
    Unchanged,
    /// The file could not be read, or could not be written back.
    Failed(&'a Error),
}

impl<'a> FileOutcome<'a> {
    /// Reads the outcome off one file's completed analysis.
    ///
    /// The distinction between changed and unchanged is the byte comparison the
    /// analysis itself made, so the label a host aggregates and the exit status
    /// the run reports cannot disagree about whether a file drifted.
    #[must_use]
    pub fn of(result: &'a anyhow::Result<(FileReport, String)>) -> Self {
        match result {
            Ok((report, _)) if report.is_changed => Self::Changed,
            Ok(_) => Self::Unchanged,
            Err(error) => Self::Failed(error),
        }
    }
}

/// Records one completed run, or one that failed before it finished.
///
/// The status is the run's outcome as a user sees it: `success`, `drift`, or
/// `error`. A run that failed at the process boundary is recorded as `error`
/// rather than left out, so a host counts runs rather than successful runs.
pub fn record_run(mode: Mode, status: ExitStatus) {
    counter!(
        "mdtablefix_run_total",
        "mode" => mode_label(mode),
        "outcome" => status_label(status)
    )
    .increment(1);
}

/// The `mode` a metric was recorded under.
///
/// A fixed name per variant, spelled here beside the names it labels rather
/// than derived from `Debug`: a label value is part of a metric's identity for
/// every recorder that aggregates it, so a variant renamed later must not
/// silently start a second series.
fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Print => "print",
        Mode::InPlace => "in_place",
        Mode::Check => "check",
        Mode::Diff => "diff",
        Mode::ListFiles => "list_files",
    }
}

/// The `outcome` a run was recorded under: the status as a user sees it.
fn status_label(status: ExitStatus) -> &'static str {
    match status {
        ExitStatus::Success => "success",
        ExitStatus::Drift => "drift",
        ExitStatus::Error => "error",
    }
}

/// The `outcome` a single file's analysis was recorded under.
///
/// A fixed name per variant, for the same reason as [`mode_label`]: a label
/// value is part of a metric's identity for every recorder that aggregates it,
/// so a variant renamed later must not silently start a second series. The
/// analysis's own error is not a variant here — its category labels the error
/// counter instead — so the three arms stay distinct without naming files.
fn file_outcome_label(outcome: &FileOutcome<'_>) -> &'static str {
    match outcome {
        FileOutcome::Changed => "changed",
        FileOutcome::Unchanged => "unchanged",
        FileOutcome::Failed(_) => "error",
    }
}

/// Runs one file's analysis under a `debug` span, timing it and recording what
/// became of it.
///
/// The closure is the work itself, so the duration measured is the analysis's
/// and not the time the file waited to be reported: files are analysed in
/// parallel and reported in argument order, and those are not the same order.
///
/// The span carries `mode` and `outcome` under the same names and values the
/// counters use, so one filter finds a file's analysis whether it is read from
/// the trace or from the metric. Its `path` is `display_path`, the path as the
/// user wrote it: a span field is not a label, so naming the file here costs no
/// cardinality, and two files called `a.md` in different directories stay
/// distinct. A failed analysis also emits `analysis failed` with the same
/// bounded `error_category` the error counter labels with, so a trace and a
/// metric select the same failures by the same name.
#[tracing::instrument(
    level = "debug",
    skip(analyse, mode, display_path),
    fields(
        mode = mode_label(mode),
        path = %display_path,
        outcome = field::Empty,
        elapsed_seconds = field::Empty
    )
)]
pub fn record_analysis(
    mode: Mode,
    display_path: &Utf8Path,
    analyse: impl FnOnce() -> anyhow::Result<(FileReport, String)>,
) -> anyhow::Result<(FileReport, String)> {
    let started = Instant::now();
    let result = analyse();
    let outcome = FileOutcome::of(&result);
    let elapsed = started.elapsed();
    // The outcome and the duration are known only once the work has run, so
    // they are recorded on the span the attribute declared rather than set by
    // it. `Span::current()` is that span: the attribute enters it for the
    // body.
    let span = Span::current();
    span.record("outcome", file_outcome_label(&outcome));
    span.record("elapsed_seconds", elapsed.as_secs_f64());
    // The category is the one the error counter carries, derived from the
    // error's chain rather than from its message, so a trace and a metric name
    // the same failure the same way. A successful analysis emits nothing.
    if let FileOutcome::Failed(error) = &outcome {
        debug!(error_category = category(error), "analysis failed");
    }
    record_file(mode, &outcome, elapsed);

    result
}

/// Records one file's analysis, whatever became of it.
///
/// The outcome is borrowed rather than taken, because the analysis's result
/// outlives the recording of it: the caller still has to print the payload or
/// report the failure.
///
/// The duration is recorded for failures too, so a file that stalls before it
/// fails is visible in the distribution rather than missing from it, as the
/// library's replacement metrics are.
pub fn record_file(mode: Mode, outcome: &FileOutcome<'_>, elapsed: Duration) {
    // Both labels are bound once, so the two instruments below cannot be
    // written with mismatched label sets.
    let mode_label = mode_label(mode);
    let outcome_label = file_outcome_label(outcome);
    counter!(
        "mdtablefix_file_total",
        "mode" => mode_label,
        "outcome" => outcome_label
    )
    .increment(1);
    histogram!(
        "mdtablefix_file_duration_seconds",
        "mode" => mode_label,
        "outcome" => outcome_label
    )
    .record(elapsed.as_secs_f64());

    if let FileOutcome::Failed(error) = outcome {
        counter!("mdtablefix_file_error_total", "category" => category(error)).increment(1);
    }
}

/// The bounded category of a failed analysis.
///
/// Derived from the `io::ErrorKind` in the error's chain rather than from its
/// message: a kind is a closed set, while a message names the file and the
/// operating system's own wording. `declined` is this tool's own refusal: the
/// only `InvalidInput` a single file's analysis can produce is the rewrite
/// boundary refusing to replace a symlink, because a path that is not UTF-8 is
/// rejected at the command line before any file is analysed. `other` is
/// everything else, including an error with no `io::Error` in its chain, so a
/// new category cannot appear without the name changing.
fn category(error: &Error) -> &'static str {
    match error
        .chain()
        .find_map(|cause| cause.downcast_ref::<io::Error>())
        .map(io::Error::kind)
    {
        Some(io::ErrorKind::NotFound) => "not_found",
        Some(io::ErrorKind::PermissionDenied) => "permission_denied",
        Some(io::ErrorKind::InvalidInput) => "declined",
        Some(_) | None => "other",
    }
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod metrics_tests;

#[cfg(test)]
#[path = "metrics_file_tests.rs"]
mod metrics_file_tests;
