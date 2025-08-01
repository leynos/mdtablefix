//! Link handling during wrapping.

use super::*;

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

#[test]
fn test_wrap_link_with_trailing_punctuation() {
    let input = lines_vec![
        "[`rust-multithreaded-logging-framework-for-python-design.md`](./rust-multithreaded-logging-framework-for-python-design.md).",
    ];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

#[rstest]
#[case(".")]
#[case(",")]
#[case(";")]
#[case(":")]
#[case("!")]
#[case("?")]
#[case("...")]
fn test_wrap_link_with_various_trailing_punctuation(#[case] punct: &str) {
    let input = lines_vec![format!("[link](https://example.com){}", punct)];
    let output = process_stream(&input);
    assert_eq!(output, input, "Failed for punctuation: {punct}");
}

#[test]
fn test_wrap_link_at_line_end() {
    let input = lines_vec!["Check out [link](https://example.com)"];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

#[test]
fn test_wrap_link_with_punctuation_in_text() {
    let input = lines_vec!["[foo, bar!](https://example.com)"];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

#[test]
fn test_wrap_link_with_punctuation_in_url() {
    let input = lines_vec!["[link](https://example.com/foo,bar)"];
    let output = process_stream(&input);
    assert_eq!(output, input);
}
