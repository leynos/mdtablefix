//! Shared `rstest` fixtures for integration tests.

#[rstest::fixture]
#[test_macros::allow_fixture_expansion_lints]
pub fn broken_table() -> Vec<String> { crate::lines_vec!["| A | B |    |", "| 1 | 2 |  | 3 | 4 |"] }
