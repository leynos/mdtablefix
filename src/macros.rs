//! Helper macros used across the crate.

/// Lazily compile a [`Regex`] with a custom panic message.
///
/// # Examples
///
/// ```
/// use std::sync::LazyLock;
///
/// use regex::Regex;
/// static RE: LazyLock<Regex> = mdtablefix::lazy_regex!(r"\d+", "digits");
/// assert!(RE.is_match("42"));
/// ```
#[macro_export]
macro_rules! lazy_regex {
    ($pattern:expr, $msg:expr $(,)?) => {
        LazyLock::new(|| Regex::new($pattern).expect($msg))
    };
}
