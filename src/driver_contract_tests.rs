//! Unit tests for the exit-status contract and for input resolution.
//!
//! The two are tested together because they are the decisions taken before any
//! file is read: which status a run has earned, and which files it was asked
//! about. Everything downstream of a readable file is in `report_tests`.

use std::path::PathBuf;

use camino::Utf8PathBuf;
use rstest::rstest;

use super::{ExitStatus, Inputs, Mode, exit_status, in_argument_order};

/// `INV-EXIT`: every combination of mode, drift, and error maps to exactly one
/// documented status, with an error outranking drift in every mode and drift a
/// status only under the reporting mode.
#[rstest]
#[case(Mode::Print, false, false, ExitStatus::Success)]
#[case(Mode::Print, true, false, ExitStatus::Success)]
#[case(Mode::Print, false, true, ExitStatus::Error)]
#[case(Mode::Print, true, true, ExitStatus::Error)]
#[case(Mode::InPlace, false, false, ExitStatus::Success)]
#[case(Mode::InPlace, true, false, ExitStatus::Success)]
#[case(Mode::InPlace, false, true, ExitStatus::Error)]
#[case(Mode::InPlace, true, true, ExitStatus::Error)]
#[case(Mode::Check, false, false, ExitStatus::Success)]
#[case(Mode::Check, true, false, ExitStatus::Drift)]
#[case(Mode::Check, false, true, ExitStatus::Error)]
#[case(Mode::Check, true, true, ExitStatus::Error)]
#[case(Mode::Diff, false, false, ExitStatus::Success)]
#[case(Mode::Diff, true, false, ExitStatus::Drift)]
#[case(Mode::Diff, false, true, ExitStatus::Error)]
#[case(Mode::Diff, true, true, ExitStatus::Error)]
fn exit_status_covers_inv_exit(
    #[case] mode: Mode,
    #[case] any_drift: bool,
    #[case] any_error: bool,
    #[case] expected: ExitStatus,
) {
    assert_eq!(exit_status(mode, any_drift, any_error), expected);
}

/// The three statuses are the three documented codes, and nothing else.
#[test]
fn exit_status_codes_are_the_documented_three() {
    assert_eq!(ExitStatus::Success.code(), std::process::ExitCode::from(0));
    assert_eq!(ExitStatus::Drift.code(), std::process::ExitCode::from(1));
    assert_eq!(ExitStatus::Error.code(), std::process::ExitCode::from(2));
}

/// `INV-ORDER`: results are re-ordered by argument index, whatever order the
/// parallel stage happened to produce them in.
#[test]
fn in_argument_order_restores_the_argument_sequence() {
    let shuffled = vec![(2, "charlie"), (0, "alpha"), (1, "bravo")];

    assert_eq!(
        in_argument_order(shuffled),
        vec!["alpha", "bravo", "charlie"]
    );
}

/// An empty batch is a valid batch, and yields no results rather than
/// panicking on a missing first element.
#[test]
fn in_argument_order_accepts_an_empty_batch() {
    let empty: Vec<(usize, &str)> = Vec::new();

    assert_eq!(in_argument_order(empty), Vec::<&str>::new());
}

/// `AX-6`: no paths named means standard input, and says so in the type rather
/// than by leaving a list empty.
#[test]
fn resolve_reads_an_empty_argument_list_as_standard_input() {
    assert_eq!(
        Inputs::resolve(Vec::new()).expect("resolve the empty argument list"),
        Inputs::Stdin
    );
}

/// The resolved files keep argument order, so a later sort cannot quietly
/// reorder the reports the user will read.
#[test]
fn resolve_keeps_the_named_paths_in_argument_order() {
    let files = vec![PathBuf::from("b.md"), PathBuf::from("a.md")];

    assert_eq!(
        Inputs::resolve(files).expect("resolve the named files"),
        Inputs::Files(vec![Utf8PathBuf::from("b.md"), Utf8PathBuf::from("a.md"),])
    );
}

/// A path that is not valid UTF-8 cannot name a file in a `Dir` capability, so
/// resolution fails as a whole rather than as one file's error.
#[cfg(unix)]
#[test]
fn resolve_declines_a_non_utf8_path() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let invalid = PathBuf::from(OsString::from_vec(b"bad\xff.md".to_vec()));

    let error = Inputs::resolve(vec![invalid]).expect_err("a non-UTF-8 path must fail");

    let message = format!("{error:?}");
    assert!(message.contains("UTF-8"), "unexpected error: {message}");
}
