//! Shared `rstest` fixtures for integration tests.

#[test_macros::allow_fixture_expansion_lints]
#[rstest::fixture]
pub fn broken_table() -> Vec<String> { crate::lines_vec!["| A | B |    |", "| 1 | 2 |  | 3 | 4 |"] }
