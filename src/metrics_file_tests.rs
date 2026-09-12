//! Metrics tests for the binary's file boundary.
//!
//! The harness and the run boundary's tests live in `metrics_tests`; these are
//! the outcomes one file's analysis can be counted under, and the answer the
//! metric is read from.

use std::{io, time::Duration};

use anyhow::Error;
use camino::{Utf8Path, Utf8PathBuf};
use mdtablefix::report::{FileReport, LineDelta};
use rstest::rstest;
use tracing_test::traced_test;

use super::{
    FileOutcome,
    metrics_tests::{assert_labels_are_bounded, count_error, count_file, durations, recorded},
    record_analysis,
    record_file,
};
use crate::driver::Mode;

/// A drifting file is counted as changed, with the time its analysis took.
#[test]
fn a_changed_file_is_counted_with_its_duration() {
    let elapsed = Duration::from_millis(250);
    let ((), recorded) = recorded(|| record_file(Mode::Check, &FileOutcome::Changed, elapsed));

    assert_eq!(count_file(&recorded, "check", "changed"), 1);
    assert_eq!(
        durations(&recorded, "check", "changed"),
        [0.25],
        "the duration must be recorded in seconds"
    );
    assert_eq!(
        count_error(&recorded, "other"),
        0,
        "a file that was analysed is not an error"
    );
    assert_labels_are_bounded(&recorded);
}

/// A file that needs no change is counted too, so the two outcomes can be
/// compared rather than only the interesting one being visible.
#[test]
fn an_unchanged_file_is_counted_with_its_duration() {
    let elapsed = Duration::from_millis(2);
    let ((), recorded) = recorded(|| record_file(Mode::Diff, &FileOutcome::Unchanged, elapsed));

    assert_eq!(count_file(&recorded, "diff", "unchanged"), 1);
    assert_eq!(durations(&recorded, "diff", "unchanged"), [0.002]);
}

/// A failure is counted as a file with an error outcome, the failure's bounded
/// category, and — because a file that stalls before it fails is worth seeing —
/// the time the failure took.
#[rstest]
#[case::missing_file(io::ErrorKind::NotFound, "not_found")]
#[case::refused_by_the_system(io::ErrorKind::PermissionDenied, "permission_denied")]
#[case::refused_by_this_tool(io::ErrorKind::InvalidInput, "declined")]
#[case::anything_else(io::ErrorKind::UnexpectedEof, "other")]
fn a_failed_file_is_counted_by_its_category(#[case] kind: io::ErrorKind, #[case] category: &str) {
    let elapsed = Duration::from_millis(5);
    // The context is what the driver attaches, so the category is derived
    // through the chain rather than from the outermost error alone.
    let error = Error::new(io::Error::new(kind, "fixture")).context("reading file.md");
    let ((), recorded) =
        recorded(|| record_file(Mode::InPlace, &FileOutcome::Failed(&error), elapsed));

    assert_eq!(count_file(&recorded, "in_place", "error"), 1);
    assert_eq!(count_error(&recorded, category), 1);
    assert_eq!(
        durations(&recorded, "in_place", "error"),
        [0.005],
        "a failed analysis is timed as well as counted"
    );
    assert_labels_are_bounded(&recorded);
}

/// An error that carries no `io::Error` at all is not left uncounted, and does
/// not invent a category.
#[test]
fn an_error_without_an_io_cause_is_other() {
    let error = anyhow::anyhow!("no io error in this chain");
    let ((), recorded) = recorded(|| {
        record_file(Mode::Print, &FileOutcome::Failed(&error), Duration::ZERO);
    });

    assert_eq!(count_error(&recorded, "other"), 1);
    assert_labels_are_bounded(&recorded);
}

/// The outcome recorded for a file is the analysis's own answer, so a metric
/// cannot report drift the exit status denies, or the reverse.
#[test]
fn the_outcome_follows_the_analysis() {
    let report = |is_changed| -> anyhow::Result<(FileReport, String)> {
        let file = FileReport {
            display_path: Utf8PathBuf::from("docs/a.md"),
            is_changed,
            delta: LineDelta::default(),
        };

        Ok((file, String::new()))
    };

    assert!(matches!(
        FileOutcome::of(&report(true)),
        FileOutcome::Changed
    ));
    assert!(matches!(
        FileOutcome::of(&report(false)),
        FileOutcome::Unchanged
    ));

    let failed: anyhow::Result<(FileReport, String)> = Err(anyhow::anyhow!("unreadable"));
    assert!(matches!(FileOutcome::of(&failed), FileOutcome::Failed(_)));
}

/// A failure is named in the trace by the same bounded category the error
/// counter labels with, so a span filter and a metric filter select the same
/// failures.
#[test]
#[traced_test]
fn a_failed_analysis_names_its_category_in_the_trace() {
    // The same chain the driver builds, so the category is derived through it
    // rather than from the outermost error alone.
    let failing = || -> anyhow::Result<(FileReport, String)> {
        Err(
            Error::new(io::Error::new(io::ErrorKind::NotFound, "fixture"))
                .context("reading missing.md"),
        )
    };

    let result = record_analysis(Mode::Check, Utf8Path::new("missing.md"), failing);

    assert!(result.is_err(), "the fixture's analysis fails");
    assert!(
        logs_contain("analysis failed"),
        "a failed analysis must say so"
    );
    assert!(
        logs_contain("not_found"),
        "the trace must name the bounded category the counter labels with"
    );
}

/// A successful analysis emits no error event, so a host filtering the trace
/// for `analysis failed` sees only failures.
#[test]
#[traced_test]
fn a_successful_analysis_emits_no_failure() {
    let succeeding = || -> anyhow::Result<(FileReport, String)> {
        Ok((
            FileReport {
                display_path: Utf8PathBuf::from("docs/a.md"),
                is_changed: true,
                delta: LineDelta::default(),
            },
            String::new(),
        ))
    };

    let result = record_analysis(Mode::Check, Utf8Path::new("docs/a.md"), succeeding);

    assert!(result.is_ok(), "the fixture's analysis succeeds");
    assert!(
        !logs_contain("analysis failed"),
        "only a failure may emit the failure event"
    );
}
