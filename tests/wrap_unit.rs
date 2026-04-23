//! Unit tests for `wrap_text`.
//!
//! This module covers the core wrapping behaviour for prose and the regression
//! guards for issue `#261`, ensuring verbatim code blocks remain untouched.

use mdtablefix::wrap::wrap_text;
use rstest::rstest;
use unicode_width::UnicodeWidthStr;

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
            "volutpat."
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
        ]
    );
}

#[test]
fn wrap_text_unmatched_backticks() {
    let input = vec!["This has a `dangling code span.".to_string()];
    let wrapped = wrap_text(&input, 20);
    assert_eq!(
        wrapped,
        vec!["This has a".to_string(), "`dangling code span.".to_string()]
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

/// Guards issue `#261` by asserting both fenced and four-space indented shell
/// blocks remain byte-identical after `wrap_text` processes surrounding
/// Markdown.
#[rstest]
#[case(vec![
    "## Verification".to_string(),
    String::new(),
    "```bash".to_string(),
    "set -o pipefail".to_string(),
    "make check-fmt 2>&1 | tee /tmp/fmt.log".to_string(),
    "make lint 2>&1 | tee /tmp/lint.log".to_string(),
    "make test 2>&1 | tee /tmp/test.log".to_string(),
    "```".to_string(),
])]
#[case(vec![
    "## Verification".to_string(),
    String::new(),
    "    set -o pipefail".to_string(),
    "    make check-fmt 2>&1 | tee /tmp/fmt.log".to_string(),
    "    make lint 2>&1 | tee /tmp/lint.log".to_string(),
    "    make test 2>&1 | tee /tmp/test.log".to_string(),
])]
fn wrap_text_preserves_shell_block_after_heading(#[case] input: Vec<String>) {
    assert_eq!(wrap_text(&input, 80), input);
}

/// Guards issue `#261` by asserting fenced shell blocks remain byte-identical
/// even when the heading is immediately followed by the opening fence.
#[test]
fn wrap_text_preserves_fenced_shell_block_without_blank_line_after_heading() {
    let input = vec![
        "## Verification".to_string(),
        "```bash".to_string(),
        "set -o pipefail".to_string(),
        "make check-fmt 2>&1 | tee /tmp/fmt.log".to_string(),
        "make lint 2>&1 | tee /tmp/lint.log".to_string(),
        "make test 2>&1 | tee /tmp/test.log".to_string(),
        "```".to_string(),
    ];

    assert_eq!(wrap_text(&input, 80), input);
}

#[test]
fn wrap_text_does_not_overflow_after_tail_rebalancing() {
    let wrapped = wrap_text(&["a four five".to_string()], 6);

    assert_eq!(wrapped.join(""), "a four five");
    assert!(
        wrapped
            .iter()
            .all(|line| UnicodeWidthStr::width(line.as_str()) <= 6)
    );
}
