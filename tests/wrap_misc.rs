//! Miscellaneous tests for wrapping hard line breaks, CLI integration, and links.

use mdtablefix::process_stream;

#[macro_use]
mod prelude;
use prelude::*;

/// Tests that lines with hard line breaks (trailing spaces) are preserved after processing.
///
/// Ensures that the `process_stream` function does not remove or alter lines ending with Markdown
/// hard line breaks.
#[test]
fn test_preserve_hard_line_breaks() {
    let input = lines_vec!["Line one with break.  ", "Line two follows."];
    let output = process_stream(&input);
    assert_eq!(output.len(), 2);
    assert_eq!(output[0], "Line one with break.");
    assert_eq!(output[1], "Line two follows.");
}

/// Tests wrapping behaviour with backslash hard line breaks.
///
/// Verifies that lines ending with backslashes are handled correctly
/// according to Markdown hard line break rules.
#[test]
fn test_wrap_hard_linebreak_backslash() {
    let input: Vec<String> = include_lines!("data/hard_linebreak_input.txt");
    let expected: Vec<String> = include_lines!("data/hard_linebreak_expected.txt");
    assert_eq!(process_stream(&input), expected);
}

/// Tests edge cases for backslash hard line break handling.
///
/// Verifies correct processing of various backslash scenarios including
/// multiple backslashes, isolated backslashes, and trailing spaces.
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

/// Tests that the CLI `--wrap` option enables wrapping functionality.
///
/// Verifies that when the `--wrap` flag is provided, the CLI tool wraps
/// long lines at 80 characters and produces multi-line output.
#[test]
fn test_cli_wrap_option() {
    let input = "This line is deliberately made much longer than eighty columns so that the \
                 wrapping algorithm is forced to insert a soft line-break somewhere in the middle \
                 of the paragraph when the --wrap flag is supplied.";
    let output = Command::cargo_bin("mdtablefix")
        .expect("Failed to create cargo command for mdtablefix")
        .arg("--wrap")
        .write_stdin(format!("{input}\n"))
        .output()
        .expect("Failed to execute mdtablefix command");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.lines().count() > 1,
        "expected wrapped output on multiple lines"
    );
    assert!(text.lines().all(|l| l.len() <= 80));
}

/// Ensures that links are not split across lines when wrapping paragraphs.
#[test]
fn test_wrap_paragraph_with_link() {
    let input = lines_vec![concat!(
        "**Wireframe** is an experimental Rust library that simplifies building",
        " servers and clients for custom binary protocols. The design borrows ",
        "heavily from [Actix Web](https://actix.rs/) to provide a familiar, ",
        "declarative API for routing, extractors, and middleware."
    )];
    let output = process_stream(&input);
    assert!(
        output
            .iter()
            .any(|line| line.contains("[Actix Web](https://actix.rs/)")),
        "link should not be broken across lines"
    );
}

/// Ensures that image links are not split across lines when wrapping paragraphs.
#[test]
fn test_wrap_paragraph_with_image_link() {
    let input = lines_vec![concat!(
        "Here is an image ![logo](https://example.com/logo.png) embedded in ",
        "a sentence that should wrap without splitting the link."
    )];
    let output = process_stream(&input);
    assert!(
        output
            .iter()
            .any(|line| line.contains("![logo](https://example.com/logo.png)")),
        "image link should not be broken across lines",
    );
}

/// Ensures that links with nested parentheses are preserved during wrapping.
#[test]
fn test_wrap_paragraph_with_nested_link() {
    let input = lines_vec![concat!(
        "Check [docs](https://example.com/rust(nightly)/guide) for details on",
        " nightly features and usage."
    )];
    let output = process_stream(&input);
    assert!(
        output
            .iter()
            .any(|line| line.contains("(https://example.com/rust(nightly)/guide)")),
        "link with nested parentheses should remain intact",
    );
}
