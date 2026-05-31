//! Link handling during wrapping.
//!
//! These integration-style cases cover Markdown links and image links in prose
//! reflowed through `process_stream`. They protect link token boundaries,
//! nested URL parentheses, and trailing punctuation that must stay attached to
//! links instead of being orphaned during wrapping.

use rstest::rstest;

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
    let input = lines_vec![concat!(
        "[`rust-multithreaded-logging-framework-for-python-design.md`](./",
        "rust-multithreaded-logging-framework-for-python-design.md).",
    )];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

/// Ensures that punctuation following a wrapped link is not orphaned.
#[test]
fn test_wrap_long_link_trailing_punctuation() {
    let input = lines_vec![
        "See [HTML table support for more \
         details](docs/architecture.md#html-table-support-in-mdtablefix).",
    ];
    let expected = lines_vec![
        "See",
        "[HTML table support for more \
         details](docs/architecture.md#html-table-support-in-mdtablefix).",
    ];
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

/// Ensures that multiple trailing punctuation marks after a wrapped link are not orphaned.
#[test]
fn test_wrap_link_multiple_trailing_punctuation() {
    let input = lines_vec![concat!(
        "Check this [link](foo.md)!? Additional words are added so this line ",
        "exceeds the eighty character limit for wrapping."
    )];
    let output = process_stream(&input);
    assert!(output.iter().any(|l| l.contains("[link](foo.md)!?")));
    assert!(!output.iter().any(|l| l.trim() == "!?"));
}

/// Ensures that punctuation after wrapped links remains attached to the link.
#[rstest]
#[case(
    concat!(
        "Reference [doc](bar.md): an extended line that ensures the wrapping ",
        "logic triggers because it exceeds eighty characters easily."
    ),
    "[doc](bar.md):",
    ":"
)]
#[case(
    concat!(
        "See [note](baz.md)... this is another line with more than enough ",
        "content to exceed the default width."
    ),
    "[note](baz.md)...",
    "..."
)]
#[case(
    concat!(
        "Alert [warn](warn.md); pay attention to the guidelines as they are ",
        "critical for understanding."
    ),
    "[warn](warn.md);",
    ";"
)]
fn test_wrap_link_various_wrapped_trailing_punctuation(
    #[case] input_line: &str,
    #[case] expected_link_with_punct: &str,
    #[case] orphan_punct: &str,
) {
    let input = lines_vec![input_line];
    let output = process_stream(&input);
    assert!(
        output.iter().any(|l| l.contains(expected_link_with_punct)),
        "expected {expected_link_with_punct:?} in {output:?}",
    );
    assert!(
        !output.iter().any(|l| l.trim() == orphan_punct),
        "punctuation {orphan_punct:?} was orphaned in {output:?}",
    );
}

/// Ensures that punctuation before a link is handled correctly.
#[rstest]
#[case(
    concat!(
        "\"[Quoted link](quote.md)\" is important for understanding the ",
        "overall design because it provides context to the guidelines."
    ),
    "\"[Quoted link](quote.md)"
)]
#[case(
    "([Parenthesized link](paren.md)) is here.",
    "([Parenthesized link](paren.md)"
)]
fn test_wrap_link_leading_punctuation(#[case] input_line: &str, #[case] expected_prefix: &str) {
    let input = lines_vec![input_line];
    let output = process_stream(&input);
    assert!(
        output.iter().any(|l| l.starts_with(expected_prefix)),
        "expected a line starting with {expected_prefix:?} in {output:?}",
    );
    assert!(
        !output.iter().any(|l| l.trim() == "\""),
        "stray quote was orphaned in {output:?}",
    );
}

/// Ensures that both leading and trailing punctuation around a link are handled.
#[rstest]
#[case(
    concat!(
        "\"[Link](foo.md)!\" demonstrates punctuation around a link and ",
        "includes plenty of extra words to exceed the wrapping limit."
    ),
    "[Link](foo.md)!\""
)]
#[case(
    "([Another](bar.md)?) should remain on one line.",
    "([Another](bar.md)?)"
)]
fn test_wrap_link_leading_and_trailing_punctuation(
    #[case] input_line: &str,
    #[case] expected_link_with_punct: &str,
) {
    let input = lines_vec![input_line];
    let output = process_stream(&input);
    assert!(
        output.iter().any(|l| l.contains(expected_link_with_punct)),
        "expected {expected_link_with_punct:?} in {output:?}",
    );
    assert!(
        !output.iter().any(|l| l.trim() == "\""),
        "stray quote was orphaned in {output:?}",
    );
}
