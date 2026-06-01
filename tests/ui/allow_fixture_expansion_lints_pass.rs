//! Compile-time smoke test for `allow_fixture_expansion_lints`.

#[test_macros::allow_fixture_expansion_lints]
#[rstest::fixture]
pub fn sample_fixture() -> Vec<String> { vec!["line".to_string()] }

fn main() {
    let _ = sample_fixture();
}
