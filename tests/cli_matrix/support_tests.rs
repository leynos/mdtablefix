//! Unit tests for CLI-matrix support helpers.

use rstest::rstest;

use super::{BaseCase, TransformFlag, has_flag, is_case_id, non_wrap_signature};

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

#[test]
fn non_wrap_signature_ignores_wrap_variant() {
    let flags = [TransformFlag::Renumber, TransformFlag::Fences];
    let (unwrapped, wrapped) = (false, true);
    assert_ne!(unwrapped, wrapped);
    assert_eq!(
        non_wrap_signature("fixture.dat", &flags),
        non_wrap_signature("fixture.dat", &flags)
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
    };
    assert_eq!(has_flag(&case, flag), expected);
}
