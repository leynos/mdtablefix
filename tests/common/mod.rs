//! Utility helpers shared across integration tests.

/// Build a `Vec<String>` from a list of string slices.
///
/// This macro is primarily used in tests to reduce boilerplate when
/// constructing example tables or other collections of lines.
#[allow(unused_macros)]
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
#[allow(unused_macros)]
macro_rules! include_lines {
    ($path:literal $(,)?) => {{
        const _TXT: &str = include_str!($path);
        _TXT.lines().map(str::to_owned).collect()
    }};
}

/// Assert common wrapping expectations for list items.
///
/// Verifies the number of lines, prefix on the first line, length of all lines,
/// and indentation of continuation lines.
#[allow(dead_code)]
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
#[allow(dead_code)]
pub fn assert_wrapped_blockquote(output: &[String], prefix: &str, expected: usize) {
    assert!(!output.is_empty(), "output slice is empty");
    assert_eq!(output.len(), expected);
    assert!(output.iter().all(|l| l.starts_with(prefix)));
    assert!(output.iter().all(|l| l.len() <= 80));
}
