//! Unit tests for CLI-matrix support helpers.

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt as _;
use std::process::ExitStatus;

use assert_cmd::Command;
use rstest::rstest;

use super::{BaseCase, TransformFlag, has_flag, is_case_id, non_wrap_signature, status_text};

#[test]
fn status_text_renders_a_successful_exit_as_code_zero() {
    // `ExitStatus::default()` is documented as "successful completion", and
    // is the only success status std will hand out without spawning.
    assert_eq!(status_text(ExitStatus::default()), "code: 0");
}

#[test]
fn status_text_renders_a_failing_exit_as_its_code() {
    // An unknown argument is rejected before any work happens, which is the
    // cheapest portable source of a real non-zero status.
    let status = Command::cargo_bin("mdtablefix")
        .expect("create mdtablefix test command")
        .arg("--not-a-real-flag")
        .output()
        .expect("run mdtablefix with an unknown flag")
        .status;
    let code = status.code().expect("a rejected invocation exits normally");
    assert_ne!(code, 0, "an unknown flag should be rejected");
    assert_eq!(status_text(status), format!("code: {code}"));
}

#[cfg(unix)]
#[test]
fn status_text_names_a_signal_terminated_status() {
    // Raw wait status 15: killed by SIGTERM, with no core dump. The child
    // never exited, so there is no code to report.
    let status = ExitStatus::from_raw(15);
    assert_eq!(status.code(), None);
    assert_eq!(status_text(status), "no exit code");
}

#[rstest]
#[case("row_001", true)]
#[case("row-001", true)]
#[case("abc123", true)]
#[case("", false)]
#[case("Row_001", false)]
#[case("row 001", false)]
fn is_case_id_returns_expected_value(#[case] id: &str, #[case] expected: bool) {
    assert_eq!(is_case_id(id), expected);
}

/// The signature is the fixture and the transform flags, spelled out, which is
/// what lets `matrix_cases_expand_to_wrapped_and_unwrapped` group a wrapped row
/// with its unwrapped twin: `wrap` is not a parameter, so it cannot appear in
/// the key. The invariance itself is that test's assertion; this one pins the
/// encoding it groups by, which a signature dropping a flag or the separator
/// would otherwise satisfy while collapsing distinct rows together.
#[test]
fn non_wrap_signature_encodes_the_fixture_and_flags() {
    assert_eq!(non_wrap_signature("fixture.dat", &[]), "fixture.dat:");
    assert_eq!(
        non_wrap_signature("fixture.dat", &[TransformFlag::Renumber]),
        "fixture.dat:--renumber"
    );
    assert_eq!(
        non_wrap_signature(
            "fixture.dat",
            &[TransformFlag::Renumber, TransformFlag::Fences]
        ),
        "fixture.dat:--renumber,--fences"
    );
}

#[test]
fn non_wrap_signature_distinguishes_flag_lists() {
    assert_ne!(
        non_wrap_signature("fixture.dat", &[TransformFlag::Renumber]),
        non_wrap_signature("fixture.dat", &[TransformFlag::Fences])
    );
}

#[rstest]
#[case(TransformFlag::Renumber, true)]
#[case(TransformFlag::Fences, false)]
fn has_flag_returns_expected_value(#[case] flag: TransformFlag, #[case] expected: bool) {
    let case = BaseCase {
        id: "row_001",
        fixture: "fixture.dat",
        flags: &[TransformFlag::Renumber],
        reporting: &[],
    };
    assert_eq!(has_flag(&case, flag), expected);
}
