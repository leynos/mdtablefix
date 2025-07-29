//! Tests for wrapping of blockquotes.
//!
//! Exercises single and nested quotes plus odd prefixes.

use mdtablefix::process_stream;

#[macro_use]
#[path = "../prelude/mod.rs"]
mod prelude;
use prelude::*;

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
    assert_wrapped_blockquote(&output, "> > ", 2);
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
         indentation handling.",
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

#[test]
fn test_wrap_blockquote_short() {
    let input = lines_vec!["> short"];
    let output = process_stream(&input);
    assert_eq!(output, input);
}
