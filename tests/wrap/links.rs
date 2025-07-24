//! Link handling during wrapping.

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
#[test]
fn test_wrap_link_various_wrapped_trailing_punctuation() {
    let input = lines_vec![
        concat!(
            "Reference [doc](bar.md): an extended line that ensures the wrapping ",
            "logic triggers because it exceeds eighty characters easily."
        ),
        concat!(
            "See [note](baz.md)... this is another line with more than enough ",
            "content to exceed the default width."
        ),
        concat!(
            "Alert [warn](warn.md); pay attention to the guidelines as they are ",
            "critical for understanding."
        ),
    ];
    let output = process_stream(&input);
    assert!(output.iter().any(|l| l.contains("[doc](bar.md):")));
    assert!(output.iter().any(|l| l.contains("[note](baz.md)...")));
    assert!(output.iter().any(|l| l.contains("[warn](warn.md);")));
    assert!(!output.iter().any(|l| {
        let t = l.trim();
        t == ":" || t == "..." || t == ";"
    }));
}

/// Ensures that punctuation before a link is handled correctly.
#[test]
fn test_wrap_link_leading_punctuation() {
    let input = lines_vec![
        concat!(
            "\"[Quoted link](quote.md)\" is important for understanding the ",
            "overall design because it provides context to the guidelines."
        ),
        "([Parenthesized link](paren.md)) is here.",
    ];
    let output = process_stream(&input);
    assert!(
        output
            .iter()
            .any(|l| l.starts_with("\"[Quoted link](quote.md)"))
    );
    assert!(!output.iter().any(|l| l.trim() == "\""));
}

/// Ensures that both leading and trailing punctuation around a link are handled.
#[test]
fn test_wrap_link_leading_and_trailing_punctuation() {
    let input = lines_vec![
        concat!(
            "\"[Link](foo.md)!\" demonstrates punctuation around a link and ",
            "includes plenty of extra words to exceed the wrapping limit."
        ),
        "([Another](bar.md)?) should remain on one line.",
    ];
    let output = process_stream(&input);
    assert!(output.iter().any(|l| l.contains("[Link](foo.md)!\"")));
    assert!(!output.iter().any(|l| l.trim() == "\""));
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
