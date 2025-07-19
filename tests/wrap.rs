//! Integration tests for wrapping behaviour.
//!
//! Covers paragraphs, list items, blockquotes and footnotes,
//! including the `--wrap` CLI option.

use mdtablefix::process_stream;
use rstest::rstest;

mod prelude;
use prelude::*;
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

#[test]
fn test_wrap_list_item() {
    let input = lines_vec![
        r"- This bullet item is exceptionally long and must be wrapped to keep prefix formatting intact.",
    ];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "- ", 2);
}

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
    common::assert_wrapped_list_item(&output, prefix, expected);
}

#[test]
fn test_wrap_preserves_inline_code_spans() {
    let input = lines_vec![
        "- `script`: A multi-line script declared with the YAML `|` block style. The entire block \
         is passed to an interpreter. If the first line begins with `#!`, Netsuke executes the \
         script verbatim, respecting the shebang.",
    ];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "- ", 3);
}

#[test]
fn test_wrap_multi_backtick_code() {
    let input = lines_vec![
        "- ``cmd`` executes ```echo``` output with ``json`` format and prints results to the \
         console",
    ];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "- ", 2);
}

#[test]
fn test_wrap_multiple_inline_code_spans() {
    let input = lines_vec![
        "- Use `foo` and `bar` inside ``baz`` for testing with additional commentary to exceed \
         wrapping width",
    ];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "- ", 2);
}
#[test]
fn test_wrap_long_inline_code_item() {
    let input = lines_vec![concat!(
        "- `async def on_unhandled(self, ws: WebSocketLike, message: Union[str, bytes])`:",
        " A fallback handler for messages that are not dispatched by the more specific",
        " message handlers. This can be used for raw text/binary data or messages that",
        " don't conform to the expected structured format."
    )];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "- ", 4);
    assert!(
        output
            .first()
            .expect("output should not be empty")
            .ends_with("`:")
    );
}

#[test]
fn test_wrap_future_attribute_punctuation() {
    let input = vec![
        concat!(
            "- Test function (`#[awt]`) or a specific `#[future]` argument ",
            "(`#[future(awt)]`), tells `rstest` to automatically insert `.await` ",
            "calls for those futures."
        )
        .to_string(),
    ];
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

#[test]
fn test_wrap_footnote_multiline() {
    let input = lines_vec![concat!(
        "[^note]: This footnote is sufficiently long to require wrapping ",
        "across multiple lines so we can verify indentation."
    )];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "[^note]: ", 2);
}

#[test]
fn test_wrap_footnote_with_inline_code() {
    let input = lines_vec![concat!(
        "  [^code_note]: A footnote containing inline `code` that should wrap ",
        "across multiple lines without breaking the span."
    )];
    let output = process_stream(&input);
    common::assert_wrapped_list_item(&output, "  [^code_note]: ", 2);
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

#[test]
/// Verifies that short list items are not wrapped or altered by the stream processing logic.
///
/// Ensures that a single-line bullet list item remains unchanged after processing.
fn test_wrap_short_list_item() {
    let input = lines_vec!["- short item"];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

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

#[test]
fn test_wrap_blockquote_nested() {
    let input = lines_vec![concat!(
        "> > This nested quote contains enough text to require wrapping so that we ",
        "can verify multi-level handling."
    )];
    let output = process_stream(&input);
    common::assert_wrapped_blockquote(&output, "> > ", 2);
    let joined = output
        .iter()
        .map(|l| l.trim_start_matches("> > "))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(joined, input[0].trim_start_matches("> > "));
}

#[test]
fn test_wrap_blockquote_mixed_indentation() {
    let input = lines_vec![
        "> \t> \tThis blockquote uses both spaces and tabs in the prefix to test mixed \
         indentation handling."
    ];
    let output = process_stream(&input);
    common::assert_wrapped_blockquote(&output, "> \t> \t", 2);
    let joined = output
        .iter()
        .map(|l| l.trim_start_matches("> \t> \t"))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(joined, input[0].trim_start_matches("> \t> \t"));
}

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
    common::assert_wrapped_blockquote(&output[..3], "> ", 3);
    common::assert_wrapped_blockquote(&output[4..], "> ", 3);
}

#[test]
fn test_wrap_blockquote_extra_whitespace() {
    let input = lines_vec![
        ">    Extra spacing should not prevent correct wrapping of this quoted text that exceeds \
         the line width.",
    ];
    let output = process_stream(&input);
    common::assert_wrapped_blockquote(&output, ">    ", 2);
    let joined = output
        .iter()
        .map(|l| l.trim_start_matches(">    "))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(joined, input[0].trim_start_matches(">    "));
}

#[test]
fn test_wrap_blockquote_short() {
    let input = lines_vec!["> short"];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

#[test]
/// Tests that lines with hard line breaks (trailing spaces) are preserved after processing.
///
/// Ensures that the `process_stream` function does not remove or alter lines ending with Markdown
/// hard line breaks.
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
