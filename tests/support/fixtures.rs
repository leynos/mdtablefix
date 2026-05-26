//! Shared `rstest` fixtures for integration tests.

#[rstest::fixture]
#[rustfmt::skip]
pub fn broken_table() -> Vec<String> {
    crate::lines_vec!["| A | B |    |", "| 1 | 2 |  | 3 | 4 |"]
}
