//! Utility helpers shared across integration tests.

/// Collect a list of string-like values into a `Vec<String>`.
///
/// Useful for building small inline datasets without verbose `.to_string()`
/// calls.
macro_rules! string_vec {
    ( $($elem:expr),* $(,)? ) => {
        vec![ $( ::std::string::ToString::to_string(&$elem) ),* ]
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

    let mut open: Option<usize> = None;
    for line in output {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '`' {
                let mut len = 0;
                while i < chars.len() && chars[i] == '`' {
                    len += 1;
                    i += 1;
                }
                if let Some(open_len) = open {
                    if open_len == len {
                        open = None;
                    }
                } else {
                    open = Some(len);
                }
            } else {
                i += 1;
            }
        }
        assert!(open.is_none(), "code span split across lines");
    }
    assert!(open.is_none(), "unclosed code span");
}

/// Assert that every line in a blockquote starts with the given prefix and is at most 80
/// characters.
pub fn assert_wrapped_blockquote(output: &[String], prefix: &str, expected: usize) {
    assert!(!output.is_empty(), "output slice is empty");
    assert_eq!(output.len(), expected);
    assert!(output.iter().all(|l| l.starts_with(prefix)));
    assert!(output.iter().all(|l| l.len() <= 80));
}
