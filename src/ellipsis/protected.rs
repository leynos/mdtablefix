//! Detect literal inline regions that ellipsis replacement must not alter.
//!
//! This module owns the conservative policy for links, autolinks, bare URLs,
//! and filesystem-like tokens. It returns source ranges rather than rewritten
//! text so the caller can preserve protected bytes exactly and normalize only
//! the prose between them.

use std::ops::Range;

use tracing::{Level, trace};

use crate::wrap::{has_odd_backslash_escape_bytes, link_or_image_span};

pub(super) fn literal_spans(text: &str) -> Vec<Range<usize>> {
    let mut spans = markdown_spans(text);
    spans.extend(semantic_token_spans(text));
    merge_spans(spans)
}

fn markdown_spans(text: &str) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    for (index, character) in text.char_indices() {
        if matches!(character, '[' | '!') {
            if let Some(span) = link_or_image_span(text, index) {
                spans.push(span);
            }
        } else if character == '<'
            && let Some(span) = autolink_span(text, index)
        {
            spans.push(span);
        }
    }
    spans
}

fn autolink_span(text: &str, start: usize) -> Option<Range<usize>> {
    if has_odd_backslash_escape_bytes(text.as_bytes(), start) {
        return None;
    }
    let relative_end = text[start..].find('>')?;
    let end = start + relative_end + '>'.len_utf8();
    let content = &text[start + '<'.len_utf8()..end - '>'.len_utf8()];
    is_uri_autolink(content)
        .then_some(start..end)
        .or_else(|| is_email_autolink(content).then_some(start..end))
}

fn is_uri_autolink(content: &str) -> bool {
    let Some((scheme, destination)) = content.split_once(':') else {
        return false;
    };
    let mut scheme_chars = scheme.chars();
    scheme.len() >= 2
        && scheme.len() <= 32
        && scheme_chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
        && scheme_chars
            .all(|character| character.is_ascii_alphanumeric() || "+.-".contains(character))
        && !destination.is_empty()
        && content.chars().all(is_autolink_character)
}

fn is_email_autolink(content: &str) -> bool {
    let Some((local, domain)) = content.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && content.chars().all(is_autolink_character)
}

fn is_autolink_character(character: char) -> bool {
    !character.is_whitespace() && !character.is_control() && !matches!(character, '<' | '>')
}

fn semantic_token_spans(text: &str) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut token_start = None;
    for (index, character) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        if character.is_whitespace() {
            if let Some(start) = token_start.take()
                && is_semantic_token(&text[start..index])
            {
                spans.push(start..index);
            }
        } else if token_start.is_none() {
            token_start = Some(index);
        }
    }
    spans
}

fn is_semantic_token(token: &str) -> bool {
    if !token.contains("...") {
        return false;
    }

    let kind = if looks_like_bare_url(token) {
        Some("bare_url")
    } else if looks_like_path(token) {
        Some("filesystem_path")
    } else {
        None
    };
    if let Some(kind) = kind
        && tracing::enabled!(Level::TRACE)
    {
        trace!(
            token_length = token.chars().count(),
            kind, "protected semantic ellipsis token"
        );
    }
    kind.is_some()
}

fn looks_like_bare_url(token: &str) -> bool {
    let token = token.trim_start_matches(is_wrapper);
    is_uri_autolink(token) || token.starts_with("www.")
}

fn looks_like_path(token: &str) -> bool {
    let token = token.trim_start_matches(is_wrapper);
    token.starts_with('/')
        || token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with("~/")
        || is_windows_drive_path(token)
}

fn is_wrapper(character: char) -> bool { matches!(character, '(' | '[' | '{' | '"' | '\'') }

fn is_windows_drive_path(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

fn merge_spans(mut spans: Vec<Range<usize>>) -> Vec<Range<usize>> {
    spans.sort_by_key(|span| (span.start, span.end));
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(spans.len());
    for span in spans {
        if let Some(previous) = merged.last_mut()
            && span.start <= previous.end
        {
            previous.end = previous.end.max(span.end);
        } else {
            merged.push(span);
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    //! Tests for detecting ellipsis-protected Markdown spans.

    use proptest::prelude::*;
    use tracing_test::traced_test;

    use super::*;

    #[rstest::rstest]
    #[case::prose("wait...", Vec::<&str>::new())]
    #[case::link("[wait...](target)", vec!["[wait...](target)"])]
    #[case::url("see https://example.com/a...b", vec!["https://example.com/a...b"])]
    #[case::escaped_autolink(r"\<https://example.com/a...b>", Vec::<&str>::new())]
    #[case::path("open ./a/.../b next", vec!["./a/.../b"])]
    #[case::slash_prose("choose and/or... input/output...", Vec::<&str>::new())]
    fn finds_literal_spans(#[case] input: &str, #[case] expected: Vec<&str>) {
        let actual = literal_spans(input)
            .into_iter()
            .map(|span| &input[span])
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[traced_test]
    #[test]
    fn semantic_classification_trace_omits_document_content() {
        let input = "./private/.../secret.txt";
        let _ = literal_spans(input);
        assert!(logs_contain("protected semantic ellipsis token"));
        assert!(logs_contain("kind=\"filesystem_path\""));
        assert!(!logs_contain(input));
    }

    proptest! {
        #[test]
        fn literal_spans_are_valid_and_disjoint(
            input in proptest::collection::vec(any::<char>(), 0..80)
                .prop_map(|characters| characters.into_iter().collect::<String>()),
        ) {
            let spans = literal_spans(&input);
            for span in &spans {
                prop_assert!(span.start <= span.end);
                prop_assert!(span.end <= input.len());
                prop_assert!(input.is_char_boundary(span.start));
                prop_assert!(input.is_char_boundary(span.end));
            }
            for pair in spans.windows(2) {
                prop_assert!(pair[0].end < pair[1].start);
            }
        }
    }
}
