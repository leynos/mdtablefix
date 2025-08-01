//! Hard line break handling tests.

use super::*;

#[test]
fn test_preserve_hard_line_breaks() {
    let input = lines_vec!["Line one with break.  ", "Line two follows."];
    let output = process_stream(&input);
    assert_eq!(output.len(), 2);
    assert_eq!(output[0], "Line one with break.");
    assert_eq!(output[1], "Line two follows.");
}

#[test]
fn test_wrap_hard_linebreak_backslash() {
    let input: Vec<String> = include_lines!("data/hard_linebreak_input.txt");
    let expected: Vec<String> = include_lines!("data/hard_linebreak_expected.txt");
    assert_eq!(process_stream(&input), expected);
}

#[test]
fn test_wrap_hard_linebreak_backslash_edge_cases() {
    let input = lines_vec!(
        "This line ends with two backslashes: \\\\",
        "This line ends with a single backslash: \\",
        " \\ ",
        "\\",
        "Text before \\ and after",
        "   \\",
        "",
    );
    let expected = lines_vec!(
        "This line ends with two backslashes: \\\\ This line ends with a single backslash:",
        "\\",
        "\\",
        "\\",
        "Text before \\ and after \\",
        "",
    );
    assert_eq!(process_stream(&input), expected);
}
