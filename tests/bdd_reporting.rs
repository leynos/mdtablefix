//! Scenario bindings for the `--check` and `--diff` specifications.
//!
//! Each binding names one scenario in `tests/features/check_mode.feature` or
//! `tests/features/diff_mode.feature`, so the feature files stay the readable
//! specification and the assertions live in `tests/steps/reporting.rs`. The
//! declarations must come before the bindings: the step registry is populated
//! as macros expand, and strict compile-time validation would otherwise report
//! every step as missing.
//!
//! Both specifications share the step definitions because they describe one
//! analysis with two renderings, and a step that differed between them would be
//! the place the two modes could silently diverge.

#[path = "steps/reporting.rs"]
mod steps;

use rstest_bdd_macros::scenario;

use crate::steps::ReportingState;

/// References each specification so Cargo recompiles this target when one
/// changes.
///
/// Cargo fingerprints a test target by its source and by every file it
/// `include_str!`s, but `#[scenario]` reads the feature file itself. Without
/// these references a feature file edited on its own leaves the previously
/// expanded binary in place, and a local `make test` can pass scenarios that no
/// longer parse — which is precisely the compile-time validation the strict
/// mode exists to provide. Touching the bindings by hand is not required of
/// anyone who edits a feature file.
const _: [&str; 2] = [
    include_str!("features/check_mode.feature"),
    include_str!("features/diff_mode.feature"),
];

/// The per-scenario state every generated test builds for itself.
#[test_macros::allow_fixture_expansion_lints]
#[rstest::fixture]
fn state() -> ReportingState { ReportingState::default() }

#[scenario(
    path = "tests/features/check_mode.feature",
    name = "A clean file reports no drift and succeeds"
)]
fn clean_file_reports_no_drift(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/check_mode.feature",
    name = "A drifting file is reported with its line counts"
)]
fn drifting_file_is_reported(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/check_mode.feature",
    name = "Every supplied file is reported in argument order"
)]
fn every_file_is_reported_in_order(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/check_mode.feature",
    name = "An unreadable file yields the error status, not the drift status"
)]
fn unreadable_file_yields_the_error_status(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/check_mode.feature",
    name = "A CRLF file needing no Markdown changes reports clean"
)]
fn crlf_file_reports_clean(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/check_mode.feature",
    name = "A byte-order-marked ragged file is not reported as clean"
)]
fn marked_ragged_file_is_not_clean(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/check_mode.feature",
    name = "In-place formatting of a drifting file still succeeds"
)]
fn in_place_formatting_still_succeeds(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/check_mode.feature",
    name = "Check mode rejects being combined with in-place mode"
)]
fn check_mode_rejects_in_place(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/diff_mode.feature",
    name = "A drifting file produces a unified diff and fails"
)]
fn drifting_file_produces_a_diff(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/diff_mode.feature",
    name = "A clean file produces no diff"
)]
fn clean_file_produces_no_diff(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/diff_mode.feature",
    name = "A drifting file under in-place formatting still succeeds"
)]
fn in_place_over_a_drifting_file_still_succeeds(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/diff_mode.feature",
    name = "Diff output is byte-identical across repeated runs"
)]
fn diff_output_is_deterministic(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/diff_mode.feature",
    name = "An unreadable file yields the error status"
)]
fn unreadable_file_yields_the_error_status_in_diff_mode(#[from(state)] _state: ReportingState) {}

#[scenario(
    path = "tests/features/diff_mode.feature",
    name = "Diff mode rejects being combined with check mode"
)]
fn diff_mode_rejects_check_mode(#[from(state)] _state: ReportingState) {}
