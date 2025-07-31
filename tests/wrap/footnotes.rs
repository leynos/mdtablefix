//! Footnote wrapping tests.
//!
//! Validates wrapping behaviour for Markdown footnotes, ensuring proper
//! indentation is maintained and inline code spans are not broken across lines.
//! Tests various footnote formats including those with URLs and code.

use super::*;

#[test]
fn test_wrap_footnote_multiline() {
    let input = lines_vec![concat!(
        "[^note]: This footnote is sufficiently long to require wrapping ",
        "across multiple lines so we can verify indentation."
    )];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "[^note]: ", 2);
}

#[test]
fn test_wrap_footnote_multiline_with_blank_lines() {
    let input = lines_vec![
        "[^note]: This footnote begins with a paragraph long enough to trigger wrapping so that indentation can be checked.",
        "",
        "    This second paragraph should also wrap correctly and remain indented.",
    ];
    let output = process_stream(&input);
    assert_eq!(output[1], "");
    assert!(output.iter().skip(2).all(|l| l.starts_with("    ")));
    assert!(output.iter().all(|l| l.len() <= 80));
}

#[test]
fn test_wrap_footnote_with_inline_code() {
    let input = lines_vec![concat!(
        "  [^code_note]: A footnote containing inline `code` that should wrap ",
        "across multiple lines without breaking the span."
    )];
    let output = process_stream(&input);
    assert_wrapped_list_item(&output, "  [^code_note]: ", 2);
}

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
