//! Helper utilities for tests.

/// Collect a list of string literals (or anything that can become a `String`)
/// into a `Vec<String>`.
#[macro_export]
macro_rules! string_vec {
    ( $($elem:expr),* $(,)? ) => {
        vec![ $( ::std::string::ToString::to_string(&$elem) ),* ]
    };
}
