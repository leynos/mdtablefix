//! Integration tests for text wrapping behaviour in Markdown content.
//!
//! This module validates the wrapping functionality of the `mdtablefix` tool,
//! including:
//! - Paragraph wrapping at 80-character boundaries
//! - List item wrapping with proper indentation preservation
//! - Blockquote wrapping with prefix maintenance
//! - Footnote wrapping with correct formatting
//! - Preservation of inline code spans during wrapping
//! - Hard line break handling
//! - CLI `--wrap` option functionality

use mdtablefix::process_stream;

#[macro_use]
mod prelude;
use prelude::*;
/// Tests that long paragraphs are wrapped at 80-character boundaries.
///
/// Verifies that a paragraph exceeding 80 characters is split into multiple
/// lines, each not exceeding the limit.
#[test]
fn test_wrap_paragraph() {
    let input = lines_vec![
        "This is a very long paragraph that should be wrapped at eighty columns so it needs to \
         contain enough words to exceed that limit.",
    ];
    let output = process_stream(&input);
    assert!(output.len() > 1);
    assert!(output.iter().all(|l| l.len() <= 80));
}

/// Ensures that a paragraph with a single word longer than 80 characters is
/// handled correctly.
#[test]
fn test_wrap_paragraph_with_long_word() {
    let long_word = "a".repeat(100);
    let input = lines_vec![&long_word];
    let output = process_stream(&input);
    assert_eq!(output.len(), 1);
    assert_eq!(output[0], long_word);
}

/// Tests that list items are wrapped whilst preserving prefix formatting.
///
/// Verifies that long bullet point items are correctly wrapped across multiple
/// lines with proper indentation maintained.
#[test]
fn test_wrap_list_item() {
    let input = lines_vec![
        r"- This bullet item is exceptionally long and must be wrapped to keep prefix formatting intact.",
    ];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "- ", 2);
}

/// Parameterised test verifying inline code wrapping for various list prefixes.
///
/// Ensures that list items with inline code spans retain prefix formatting
/// across different bullet and numbered list styles.
#[rstest]
#[case("- ", 3)]
#[case("1. ", 3)]
#[case("10. ", 3)]
#[case("100. ", 3)]
fn test_wrap_list_items_with_inline_code(#[case] prefix: &str, #[case] expected: usize) {
    let input = lines_vec![format!(
        "{prefix}`script`: A multi-line script declared with the YAML `|` block style. The entire \
         block is passed to an interpreter. If the first line begins with `#!`, Netsuke executes \
         the script verbatim, respecting the shebang."
    )];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, prefix, expected);
}

/// Tests that inline code spans are preserved during list item wrapping.
///
/// Verifies that backtick-delimited code spans remain intact when wrapping
/// long list items across multiple lines.
#[test]
fn test_wrap_preserves_inline_code_spans() {
    let input = lines_vec![
        "- `script`: A multi-line script declared with the YAML `|` block style. The entire block \
         is passed to an interpreter. If the first line begins with `#!`, Netsuke executes the \
         script verbatim, respecting the shebang.",
    ];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "- ", 3);
}

/// Tests that multi-backtick code spans are preserved during wrapping.
///
/// Verifies that code spans using multiple backticks (``cmd``, ```echo```) are
/// not broken when wrapping list items.
#[test]
fn test_wrap_multi_backtick_code() {
    let input = lines_vec![
        "- ``cmd`` executes ```echo``` output with ``json`` format and prints results to the \
         console",
    ];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "- ", 2);
}

/// Tests that multiple inline code spans are preserved during wrapping.
///
/// Verifies that list items containing multiple code spans are wrapped correctly
/// without breaking the span boundaries.
#[test]
fn test_wrap_multiple_inline_code_spans() {
    let input = lines_vec![
        "- Use `foo` and `bar` inside ``baz`` for testing with additional commentary to exceed \
         wrapping width",
    ];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "- ", 2);
}
/// Tests wrapping of list items with long inline code spans.
///
/// Verifies that list items containing lengthy code spans are wrapped
/// appropriately whilst preserving the code span integrity.
#[test]
fn test_wrap_long_inline_code_item() {
    let input = lines_vec![concat!(
        "- `async def on_unhandled(self, ws: WebSocketLike, message: Union[str, bytes])`:",
        " A fallback handler for messages that are not dispatched by the more specific",
        " message handlers. This can be used for raw text/binary data or messages that",
        " don't conform to the expected structured format."
    )];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "- ", 4);
    assert!(
        output
            .first()
            .expect("wrapped output should contain at least one line")
            .ends_with("`:")
    );
}

/// Tests wrapping for punctuation around future attribute references.
///
/// Ensures that long bullet items containing attribute syntax such as
/// `#[future]` are wrapped correctly without splitting the punctuation.
#[test]
fn test_wrap_future_attribute_punctuation() {
    let input = lines_vec![concat!(
        "- Test function (`#[awt]`) or a specific `#[future]` argument ",
        "(`#[future(awt)]`), tells `rstest` to automatically insert `.await` ",
        "calls for those futures."
    )];
    let output = process_stream(&input);
    assert_eq!(
        output,
        vec![
            "- Test function (`#[awt]`) or a specific `#[future]` argument".to_string(),
            "  (`#[future(awt)]`), tells `rstest` to automatically insert `.await` calls for"
                .to_string(),
            "  those futures.".to_string(),
        ]
    );
}

/// Tests wrapping for multi-line footnotes with correct indentation.
///
/// Verifies that long footnotes are split across lines with the footnote
/// prefix preserved.
#[test]
fn test_wrap_footnote_multiline() {
    let input = lines_vec![concat!(
        "[^note]: This footnote is sufficiently long to require wrapping ",
        "across multiple lines so we can verify indentation."
    )];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "[^note]: ", 2);
}

/// Tests that footnotes containing inline code are wrapped correctly.
///
/// Verifies that code spans within footnotes are preserved during wrapping.
#[test]
fn test_wrap_footnote_with_inline_code() {
    let input = lines_vec![concat!(
        "  [^code_note]: A footnote containing inline `code` that should wrap ",
        "across multiple lines without breaking the span."
    )];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "  [^code_note]: ", 2);
}

/// Tests that footnotes with angle-bracketed URLs are wrapped correctly.
///
/// Verifies that when a footnote line contains a URL enclosed in angle brackets,
/// the URL is moved to a new indented line beneath the footnote text.
#[test]
fn test_wrap_angle_bracket_url() {
    let input = lines_vec![concat!(
        "[^5]: Given When Then - Martin Fowler, accessed on 14 July 2025, ",
        "<https://martinfowler.com/bliki/GivenWhenThen.html>"
    )];
    let expected = lines_vec![
        "[^5]: Given When Then - Martin Fowler, accessed on 14 July 2025,",
        "      <https://martinfowler.com/bliki/GivenWhenThen.html>",
    ];
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

/// Checks that a sequence of footnotes is not altered by wrapping.
///
/// This regression test ensures that the footnote collection remains
/// unchanged when passed to `process_stream`.
#[test]
fn test_wrap_footnote_collection() {
    let input = lines_vec![
        "[^1]: <https://falcon.readthedocs.io>",
        "[^2]: <https://asgi.readthedocs.io>",
        "[^3]: <https://www.starlette.io>",
        "[^4]: <https://www.starlette.io/websockets/>",
        "[^5]: <https://channels.readthedocs.io>",
        "[^6]: <https://channels.readthedocs.io/en/stable/topics/consumers.html>",
        "[^7]: <https://fastapi.tiangolo.com/advanced/websockets/>",
        "[^8]: <https://websockets.readthedocs.io>",
    ];

    let output = process_stream(&input);
    assert_eq!(output, input);
}

/// Verifies that short list items are not wrapped or altered by the stream processing logic.
///
/// Ensures that a single-line bullet list item remains unchanged after processing.
#[test]
fn test_wrap_short_list_item() {
    let input = lines_vec!["- short item"];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

/// Tests wrapping behaviour for single-level blockquotes.
///
/// Verifies that long quoted text is wrapped onto multiple lines while
/// preserving the ">" prefix on each line.
#[test]
fn test_wrap_blockquote() {
    let input = lines_vec![
        "> **Deprecated**: A :class:`WebSocketRouter` and its `add_route` API should be used to \
         instantiate resources.",
    ];
    let output = process_stream(&input);
    assert_eq!(
        output,
        lines_vec![
            "> **Deprecated**: A :class:`WebSocketRouter` and its `add_route` API should be",
            "> used to instantiate resources.",
        ]
    );
}

/// Tests that nested blockquotes are wrapped correctly.
///
/// Verifies that multi-level blockquotes ("> > ") maintain their nesting
/// structure when wrapped across multiple lines.
#[test]
fn test_wrap_blockquote_nested() {
    let input = lines_vec![concat!(
        "> > This nested quote contains enough text to require wrapping so that we ",
        "can verify multi-level handling."
    )];
    let output = process_stream(&input);
    assert_wrapped_blockquote(&output, "> > ", 2);
    let joined = output
        .iter()
        .map(|l| l.trim_start_matches("> > "))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(joined, input[0].trim_start_matches("> > "));
}

/// Tests blockquote wrapping with mixed spaces and tabs in prefix.
///
/// Verifies that blockquotes using both spaces and tabs maintain correct
/// prefix formatting when wrapped.
#[test]
fn test_wrap_blockquote_mixed_indentation() {
    let input = lines_vec![
        "> \t> \tThis blockquote uses both spaces and tabs in the prefix to test mixed \
         indentation handling."
    ];
    let output = process_stream(&input);
    assert_wrapped_blockquote(&output, "> \t> \t", 2);
    let joined = output
        .iter()
        .map(|l| l.trim_start_matches("> \t> \t"))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(joined, input[0].trim_start_matches("> \t> \t"));
}

/// Tests blockquote wrapping with blank lines preserved.
///
/// Verifies that blank lines within blockquotes are maintained correctly when wrapping long quoted
/// paragraphs.
#[test]
fn test_wrap_blockquote_with_blank_lines() {
    let input = lines_vec![
        concat!(
            "> The first paragraph in this quote is deliberately long enough to wrap ",
            "across multiple lines so"
        ),
        "> demonstrate the behaviour.",
        ">",
        concat!(
            "> The second paragraph is also extended to trigger wrapping in order to ",
            "ensure blank lines"
        ),
        "> are preserved correctly.",
    ];
    let output = process_stream(&input);
    assert_eq!(output[3], ">");
    assert_wrapped_blockquote(&output[..3], "> ", 3);
    assert_wrapped_blockquote(&output[4..], "> ", 3);
}

/// Tests blockquote wrapping with extra spacing in prefix.
///
/// Verifies that blockquotes with additional spaces after ">" are wrapped correctly whilst
/// preserving the spacing.
#[test]
fn test_wrap_blockquote_extra_whitespace() {
    let input = lines_vec![
        ">    Extra spacing should not prevent correct wrapping of this quoted text that exceeds \
         the line width.",
    ];
    let output = process_stream(&input);
    assert_wrapped_blockquote(&output, ">    ", 2);
    let joined = output
        .iter()
        .map(|l| l.trim_start_matches(">    "))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(joined, input[0].trim_start_matches(">    "));
}

/// Tests that short blockquotes remain unchanged after processing.
///
/// Verifies that brief quoted text is not altered by the wrapping logic.
#[test]
fn test_wrap_blockquote_short() {
    let input = lines_vec!["> short"];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

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

/// Ensures that markdownlint directives remain on their own line when wrapping.
#[test]
fn test_markdownlint_directive_not_broken() {
    let input = lines_vec![
        "[roadmap](./roadmap.md) and expands on the design ideas described in",
        "<!--  markdownlint-disable-next-line  MD013  -->",
    ];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

/// Regular comments should be reflowed like ordinary text when wrapping.
#[test]
fn test_regular_comment_wraps_normally() {
    let input = lines_vec![
        "Intro text that preludes a lengthy comment.",
        concat!(
            "<!-- This comment contains many words and should be wrapped across ",
            "multiple lines to ensure that regular comments are formatted ",
            "correctly. -->"
        ),
    ];
    let output = process_stream(&input);
    assert_eq!(
        output,
        lines_vec![
            "Intro text that preludes a lengthy comment. <!-- This comment contains many",
            "words and should be wrapped across multiple lines to ensure that regular",
            "comments are formatted correctly. -->",
        ]
    );
}
