//! Unit tests for text wrapping functionality.
//!
//! This module contains tests for the `wrap_text` function, verifying correct
//! behaviour with code spans, links, hyphenated words, and various line widths.

use super::super::*;

#[test]
fn wrap_text_preserves_hyphenated_words() {
    let input = vec!["A word that is very-long-word indeed".to_string()];
    let wrapped = wrap_text(&input, 20);
    assert_eq!(
        wrapped,
        vec![
            "A word that is".to_string(),
            "very-long-word".to_string(),
            "indeed".to_string(),
        ]
    );
}

#[test]
fn wrap_text_does_not_insert_spaces_in_hyphenated_words() {
    let input = vec![
        concat!(
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Donec tincidunt ",
            "elit-sed fermentum congue. Vivamus dictum nulla sed consectetur ",
            "volutpat.",
        )
        .to_string(),
    ];
    let wrapped = wrap_text(&input, 80);
    assert_eq!(
        wrapped,
        vec![
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Donec tincidunt".to_string(),
            "elit-sed fermentum congue. Vivamus dictum nulla sed consectetur volutpat.".to_string(),
        ]
    );
}

#[test]
fn wrap_text_preserves_code_spans() {
    let input = vec![
        "with their own escaping rules. On Windows, scripts default to `powershell -Command` \
         unless the manifest's `interpreter` field overrides the setting."
            .to_string(),
    ];
    let wrapped = wrap_text(&input, 60);
    assert_eq!(
        wrapped,
        vec![
            "with their own escaping rules. On Windows, scripts default".to_string(),
            "to `powershell -Command` unless the manifest's".to_string(),
            "`interpreter` field overrides the setting.".to_string(),
        ]
    );
}

#[test]
fn wrap_text_multiple_code_spans() {
    let input = vec!["combine `foo bar` and `baz qux` in one line".to_string()];
    let wrapped = wrap_text(&input, 25);
    assert_eq!(
        wrapped,
        vec![
            "combine `foo bar` and".to_string(),
            "`baz qux` in one line".to_string(),
        ]
    );
}

#[test]
fn wrap_text_nested_backticks() {
    let input = vec!["Use `` `code` `` to quote backticks".to_string()];
    let wrapped = wrap_text(&input, 20);
    assert_eq!(
        wrapped,
        vec![
            "Use `` `code` `` to".to_string(),
            "quote backticks".to_string()
        ],
    );
}

#[test]
fn wrap_text_unmatched_backticks() {
    let input = vec!["This has a `dangling code span.".to_string()];
    let wrapped = wrap_text(&input, 20);
    assert_eq!(
        wrapped,
        vec!["This has a".to_string(), "`dangling code span.".to_string()],
    );
}

#[test]
fn wrap_text_preserves_links() {
    let input = vec![
        "`falcon-pachinko` is an extension library for the".to_string(),
        "[Falcon](https://falcon.readthedocs.io) web framework. It adds a structured".to_string(),
        "approach to asynchronous WebSocket routing and background worker integration.".to_string(),
    ];
    let wrapped = wrap_text(&input, 80);
    let joined = wrapped.join("\n");
    assert_eq!(joined.matches("https://").count(), 1);
    assert!(
        wrapped
            .iter()
            .any(|l| l.contains("https://falcon.readthedocs.io"))
    );
}

#[test]
fn wrap_preserving_code_keeps_trailing_spaces() {
    // Trailing spaces should be retained when flushing the final line.
    let lines = super::wrap_preserving_code("ends with space  ", 80);
    assert_eq!(lines, vec!["ends with space  ".to_string()]);
}
