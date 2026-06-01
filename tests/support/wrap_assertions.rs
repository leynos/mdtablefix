//! Assertion helpers for wrap integration tests.

/// Assert common wrapping expectations for list items.
///
/// Verifies the number of lines, prefix on the first line, length of all lines,
/// and indentation of continuation lines.
///
/// # Panics
///
/// Panics if the output slice is empty, expected count is zero, or if the lines
/// do not meet the asserted conditions.
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
        scan_code_spans(line, &mut open);
        assert!(open.is_none(), "code span split across lines");
    }
    assert!(open.is_none(), "unclosed code span");
}

fn scan_code_spans(line: &str, open: &mut Option<usize>) {
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '`' {
            continue;
        }

        let mut len = 1;
        while chars.next_if_eq(&'`').is_some() {
            len += 1;
        }
        toggle_code_span(open, len);
    }
}

fn toggle_code_span(open: &mut Option<usize>, len: usize) {
    if open.is_some_and(|open_len| open_len == len) {
        *open = None;
    } else if open.is_none() {
        *open = Some(len);
    }
}

/// Assert that a wrapped blockquote has the expected number of lines and prefix.
///
/// The `expected` parameter is the expected number of lines in the wrapped
/// blockquote.
///
/// # Panics
///
/// Panics if the output slice is empty, expected count is zero, or if the prefix
/// is missing from any line.
pub fn assert_wrapped_blockquote(output: &[String], prefix: &str, expected: usize) {
    assert!(expected > 0, "expected line count must be positive");
    assert!(!output.is_empty(), "output slice is empty");
    assert_eq!(output.len(), expected);
    assert!(output.iter().all(|l| l.starts_with(prefix)));
    assert!(output.iter().all(|l| l.len() <= 80));
}
