//! Integration tests for paragraph and list item wrapping behaviour.

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
