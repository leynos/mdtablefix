//! Integration tests for block-level `wrap_text` behaviour.

use rstest::rstest;

use crate::wrap::{BlockKind, classify_block, wrap_text};

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
fn wrap_text_breaks_between_space_separated_code_spans() {
    let input = vec![
        concat!(
            "The file loader selects the parser based on the extension ",
            "(`.toml`, `.json`, `.json5`, `.yaml`, `.yml`). When the `json5` ",
            "feature is active, both `.json` and `.json5` files are parsed ",
            "using the JSON5 format."
        )
        .to_string(),
    ];
    let wrapped = wrap_text(&input, 80);

    for line in &wrapped {
        assert!(
            unicode_width::UnicodeWidthStr::width(line.as_str()) <= 80,
            "line too wide ({} cols): {line:?}",
            unicode_width::UnicodeWidthStr::width(line.as_str())
        );
    }

    assert!(
        wrapped[0].ends_with("`.toml`,") || wrapped[0].ends_with("`.json`,"),
        "expected first line to break inside the code-span list, got: {:?}",
        wrapped[0]
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
fn wrap_text_keeps_trailing_spaces_for_blockquote_final_line() {
    // "> " is the prefix; available width = 10 - 2 = 8.
    let input = vec!["> word1 word2  ".to_string()];
    let wrapped = wrap_text(&input, 10);
    assert_eq!(
        wrapped,
        vec!["> word1".to_string(), "> word2  ".to_string()]
    );
}

#[test]
fn wrap_text_keeps_trailing_spaces_for_bullet_final_line() {
    // "- " is the prefix; continuation lines are indented with two spaces.
    let input = vec!["- word1 word2  ".to_string()];
    let wrapped = wrap_text(&input, 10);
    assert_eq!(
        wrapped,
        vec!["- word1".to_string(), "  word2  ".to_string()]
    );
}

#[test]
fn wrap_text_preserves_indented_hash_as_text() {
    let input = vec![
        "Paragraph intro.".to_string(),
        "    # code".to_string(),
        "Continuation.".to_string(),
    ];
    let wrapped = wrap_text(&input, 40);
    assert_eq!(
        wrapped,
        vec![
            "Paragraph intro.".to_string(),
            "    # code".to_string(),
            "Continuation.".to_string(),
        ]
    );
}

#[test]
fn wrap_text_flushes_before_heading() {
    let input = vec![
        "Paragraph intro.".to_string(),
        "# Heading".to_string(),
        "Continuation.".to_string(),
    ];
    let wrapped = wrap_text(&input, 40);
    assert_eq!(
        wrapped,
        vec![
            "Paragraph intro.".to_string(),
            "# Heading".to_string(),
            "Continuation.".to_string(),
        ]
    );
}

#[rstest(
    line,
    expected,
    case("# Heading", Some(BlockKind::Heading)),
    case("   # Heading", Some(BlockKind::Heading)),
    case("    # Heading", None),
    case("	# Heading", None),
    case("- item", Some(BlockKind::Bullet)),
    case("1. item", Some(BlockKind::Bullet)),
    case("> quote", Some(BlockKind::Blockquote)),
    case("[^1]: footnote", Some(BlockKind::FootnoteDefinition)),
    case(
        "<!-- markdownlint-disable -->",
        Some(BlockKind::MarkdownlintDirective)
    ),
    case("2024 revenue", Some(BlockKind::DigitPrefix)),
    case("a | b", None),
    case("plain text", None)
)]
fn classify_block_detects_markdown_prefixes(line: &str, expected: Option<BlockKind>) {
    assert_eq!(classify_block(line), expected);
}
