//! Link handling during wrapping.
//!
//! These integration-style cases cover Markdown links and image links in prose
//! reflowed through `process_stream`. They protect link token boundaries,
//! nested URL parentheses, and trailing punctuation that must stay attached to
//! links instead of being orphaned during wrapping.

use insta::assert_snapshot;
use mdtablefix::process::WRAP_COLS;
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

#[test]
fn test_wrap_empty_url_link_is_not_split() {
    let input = lines_vec![concat!(
        "Inspect the placeholder []() inside a long paragraph that easily ",
        "exceeds the eighty character wrap threshold for prose reflow."
    )];
    let output = process_stream(&input);
    assert!(
        output.iter().any(|l| l.contains("[]()")),
        "empty-URL link should remain intact in {output:?}",
    );
    assert!(
        !output.iter().any(|l| l.trim_start().starts_with("()")),
        "the URL parentheses must not lead a wrapped line in {output:?}",
    );
}

#[test]
fn test_wrap_link_with_unbalanced_parens_is_not_split() {
    let input = lines_vec![concat!(
        "See [example](https://example.com/path(fragment) for additional ",
        "context that pushes this paragraph beyond the eighty character limit."
    )];
    let output = process_stream(&input);
    assert!(
        output
            .iter()
            .any(|l| l.contains("[example](https://example.com/path(fragment)")),
        "unbalanced-paren link should stay intact in {output:?}",
    );
}

#[test]
fn test_wrap_link_at_exact_wrap_boundary_is_not_split() {
    const WORD: &str = "Word ";
    let prefix = WORD.repeat(WRAP_COLS / WORD.len());
    assert_eq!(prefix.len() % WRAP_COLS, 0);
    let link = "[boundary](https://example.com/wrap-boundary-test)";
    let punct = ".";
    // Trailing text pushes total length well past WRAP_COLS so wrapping fires.
    let input = lines_vec![format!(
        "{prefix}{link}{punct} trailing text to force wrapping here."
    )];
    let output = process_stream(&input);
    // The link starts at the exact wrap boundary; it must land on its own line
    // with its trailing punctuation attached — not as two separate tokens.
    let link_with_punct = format!("{link}{punct}");
    assert!(
        output.iter().any(|l| l == &link_with_punct),
        "link with punctuation should appear as its own complete line at the wrap boundary; got \
         {output:?}",
    );
    assert!(
        !output.iter().any(|l| l.trim() == punct),
        "trailing punctuation must not be orphaned onto its own line; got {output:?}",
    );
}

#[rstest]
#[case(
    "long_link_trailing_period",
    lines_vec![concat!(
        "See [HTML table support for more ",
        "details](docs/architecture.md#html-table-support-in-mdtablefix).",
    )]
)]
#[case(
    "multiple_trailing_marks",
    lines_vec![concat!(
        "Check this [link](foo.md)!? Additional words are added so this line ",
        "exceeds the eighty character limit for wrapping."
    )]
)]
#[case(
    "colon_after_link",
    lines_vec![concat!(
        "Reference [doc](bar.md): an extended line that ensures the wrapping ",
        "logic triggers because it exceeds eighty characters easily."
    )]
)]
#[case(
    "leading_quote_before_link",
    lines_vec![concat!(
        "\"[Quoted link](quote.md)\" is important for understanding the ",
        "overall design because it provides context to the guidelines."
    )]
)]
#[case(
    "leading_and_trailing_punctuation",
    lines_vec![concat!(
        "\"[Link](foo.md)!\" demonstrates punctuation around a link and ",
        "includes plenty of extra words to exceed the wrapping limit."
    )]
)]
fn snapshot_link_punctuation_wrapping(#[case] name: &str, #[case] input: Vec<String>) {
    let rendered = process_stream(&input).join("\n");
    insta::with_settings!({
        description => format!("link punctuation wrapping: {name}"),
        omit_expression => true,
    }, {
        assert_snapshot!(name, rendered);
    });
}
