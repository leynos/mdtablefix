//! Utility helpers shared across integration tests.

/// Build a `Vec<String>` from a list of string slices.
///
/// This macro is primarily used in tests to reduce boilerplate when
/// constructing example tables or other collections of lines.
macro_rules! lines_vec {
    ($($line:expr),* $(,)?) => {
        vec![$($line.to_string()),*]
    };
}

/// Assert common wrapping expectations for list items.
///
/// Verifies the number of lines, prefix on the first line, length of all lines,
/// and indentation of continuation lines.
pub fn assert_wrapped_list_item(output: &[String], prefix: &str, expected: usize) {
    assert!(expected > 0, "expected line count must be positive");
    assert!(!output.is_empty(), "output slice is empty");
    assert_eq!(output.len(), expected);
    assert!(output.first().is_some_and(|line| line.starts_with(prefix)));
    assert!(output.iter().all(|l| l.len() <= 80));
    let indent = " ".repeat(prefix.len());
    for line in output.iter().skip(1) {
        assert!(line.starts_with(&indent));
    }
}
