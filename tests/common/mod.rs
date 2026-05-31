//! Utility macros shared across integration tests.

/// Build a `Vec<String>` from a list of string slices.
///
/// This macro is primarily used in tests to reduce boilerplate when
/// constructing example tables or other collections of lines.
#[macro_export]
macro_rules! lines_vec {
    ($($line:expr),* $(,)?) => {
        vec![$($line.to_string()),*]
    };
}

/// Expands to a `Vec<String>` with one element per line of the file.
///
/// Example:
/// ```
/// let input: Vec<String> = include_lines!("data/bold_header_input.txt"); 
/// ```
#[macro_export]
macro_rules! include_lines {
    ($path:literal $(,)?) => {{
        const _TXT: &str = include_str!($path);
        _TXT.lines().map(str::to_owned).collect()
    }};
}
