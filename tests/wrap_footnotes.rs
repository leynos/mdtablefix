//! Integration tests for wrapping footnote blocks and collections.

use mdtablefix::process_stream;

#[macro_use]
mod prelude;
use prelude::*;

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
