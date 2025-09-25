//! Unit tests for text wrapping functionality.
//!
//! This module contains tests for the `wrap_text` function, verifying correct
//! behaviour with code spans, links, hyphenated words, and various line widths.

use rstest::rstest;

use super::super::*;
use super::{
    LineBuffer, append_group_to_line, determine_token_span, handle_split_overflow,
    handle_trailing_whitespace_group, start_new_line_with_group, tokenize::segment_inline,
    wrap_preserving_code,
};

#[rstest]
#[case("`code`!", "`code`!")]
#[case("[link](url).", "[link](url).")]
#[case("plain,", "plain,")]
fn determine_token_span_groups_related_tokens(#[case] input: &str, #[case] expected_group: &str) {
    let tokens = segment_inline(input);
    let (end, width) = determine_token_span(&tokens, 0);
    let grouped = tokens[..end].join("");
    assert_eq!(grouped, expected_group);
    assert_eq!(width, unicode_width::UnicodeWidthStr::width(expected_group));
}

#[test]
fn append_group_to_line_updates_last_split_for_whitespace() {
    let tokens = segment_inline("foo bar");
    let mut current = String::new();
    let mut current_width = 0;
    let mut last_split = None;

    {
        let mut buffer = LineBuffer::new(&mut current, &mut current_width, &mut last_split);
        append_group_to_line(&tokens, 0, 1, &mut buffer);
    }
    assert_eq!(current, "foo");
    assert_eq!(current_width, unicode_width::UnicodeWidthStr::width("foo"));
    assert_eq!(last_split, None);

    {
        let mut buffer = LineBuffer::new(&mut current, &mut current_width, &mut last_split);
        append_group_to_line(&tokens, 1, 2, &mut buffer);
    }
    assert_eq!(current, "foo ");
    assert_eq!(current_width, unicode_width::UnicodeWidthStr::width("foo "));
    assert_eq!(last_split, Some(current.len()));
}

#[test]
fn handle_split_overflow_moves_tokens_to_new_line() {
    let tokens = segment_inline("foo bar baz");
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    let mut last_split = None;

    for start in 0..4 {
        let (end, _) = determine_token_span(&tokens, start);
        let mut buffer = LineBuffer::new(&mut current, &mut current_width, &mut last_split);
        append_group_to_line(&tokens, start, end, &mut buffer);
    }

    let (group_end, _) = determine_token_span(&tokens, 4);
    let handled = {
        let mut buffer = LineBuffer::new(&mut current, &mut current_width, &mut last_split);
        handle_split_overflow(&mut lines, &mut buffer, &tokens, 4, group_end, 10)
    };

    assert!(handled);
    assert_eq!(lines, vec!["foo bar".to_string()]);
    assert_eq!(current, "baz");
    assert_eq!(current_width, unicode_width::UnicodeWidthStr::width("baz"));
    assert_eq!(last_split, None);
}

#[test]
fn handle_trailing_whitespace_group_preserves_spaces() {
    let tokens = segment_inline("foo  ");
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    let mut last_split = None;

    let (word_end, _) = determine_token_span(&tokens, 0);
    {
        let mut buffer = LineBuffer::new(&mut current, &mut current_width, &mut last_split);
        append_group_to_line(&tokens, 0, word_end, &mut buffer);
    }

    let start = tokens
        .iter()
        .enumerate()
        .rev()
        .find(|(_, tok)| tok.chars().all(char::is_whitespace))
        .map(|(idx, _)| idx)
        .expect("at least one trailing whitespace token");
    let (group_end, _) = determine_token_span(&tokens, start);

    let handled = {
        let mut buffer = LineBuffer::new(&mut current, &mut current_width, &mut last_split);
        handle_trailing_whitespace_group(&mut lines, &mut buffer, &tokens, start, group_end)
    };

    assert!(handled);
    assert_eq!(lines, vec!["foo  ".to_string()]);
    assert!(current.is_empty());
    assert_eq!(current_width, 0);
    assert_eq!(last_split, None);
}

#[test]
fn start_new_line_with_group_flushes_existing_line() {
    let tokens = segment_inline("baz ");
    let mut lines = Vec::new();
    let mut current = "foo".to_string();
    let mut current_width = unicode_width::UnicodeWidthStr::width("foo");
    let mut last_split = Some(current.len());

    let (group_end, _) = determine_token_span(&tokens, 0);

    {
        let mut buffer = LineBuffer::new(&mut current, &mut current_width, &mut last_split);
        start_new_line_with_group(&mut lines, &mut buffer, &tokens, 0, group_end);
    }

    assert_eq!(lines, vec!["foo".to_string()]);
    assert_eq!(current, "baz");
    assert_eq!(current_width, unicode_width::UnicodeWidthStr::width("baz"));
    assert_eq!(last_split, None);
}

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

#[rstest]
#[case("trail  ", 80, &["trail  "])]
#[case("`code span`  ", 12, &["`code span`  "])]
#[case("foo  ", 3, &["foo  "])]
#[case("x  ", 1, &["x  "])]
fn preserves_trailing_spaces(#[case] input: &str, #[case] width: usize, #[case] expected: &[&str]) {
    let out = wrap_preserving_code(input, width);
    assert_eq!(
        out,
        expected.iter().map(|&s| s.to_string()).collect::<Vec<_>>()
    );
}

#[rstest]
#[case("aaaaaaaaaaaa", 5, &["aaaaaaaaaaaa"])] // forced flush without split
#[case("abcde", 3, &["abcde"])]
#[case("`codespan`", 6, &["`codespan`"])]
fn no_split_forced_flush_no_trim(
    #[case] input: &str,
    #[case] width: usize,
    #[case] expected: &[&str],
) {
    let out = wrap_preserving_code(input, width);
    assert_eq!(
        out,
        expected.iter().map(|&s| s.to_string()).collect::<Vec<_>>()
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
