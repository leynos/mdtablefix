//! Scenario bindings for the `--check` specification.
//!
//! Each binding names one scenario in `tests/features/check_mode.feature`, so
//! the feature file stays the readable specification and the assertions live in
//! `tests/steps/reporting.rs`. The declarations must come before the bindings:
//! the step registry is populated as macros expand, and strict compile-time
//! validation would otherwise report every step as missing.

#[path = "steps/reporting.rs"]
mod steps;

use rstest_bdd_macros::scenario;

use crate::steps::ReportingState;

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
