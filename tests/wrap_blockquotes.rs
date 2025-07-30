//! Integration tests for wrapping blockquotes.

use mdtablefix::process_stream;

#[macro_use]
mod prelude;
use prelude::*;

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
