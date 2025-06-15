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
