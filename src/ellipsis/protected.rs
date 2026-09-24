//! Detect literal inline regions that ellipsis replacement must not alter.
//!
//! This module owns the conservative policy for links, autolinks, bare URLs,
//! and filesystem-like tokens. It returns source ranges rather than rewritten
//! text so the caller can preserve protected bytes exactly and normalize only
//! the prose between them.

use std::ops::Range;

use tracing::{Level, trace};

use crate::wrap::{has_odd_backslash_escape_bytes, link_or_image_span};

/// Returns non-overlapping source ranges whose ellipses must survive unchanged.
///
/// Markdown links and autolinks are combined with semantic URL/path tokens because both forms can
/// contain literal dots that prose normalisation must not rewrite.
pub(super) fn literal_spans(text: &str) -> Vec<Range<usize>> {
    let mut spans = markdown_spans(text);
    spans.extend(semantic_token_spans(text));
    merge_spans(spans)
}

/// Finds links, images, and angle-bracket autolinks using source byte ranges.
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

/// Recognises an unescaped URI or e-mail autolink beginning at the supplied byte offset.
fn autolink_span(text: &str, start: usize) -> Option<Range<usize>> {
    if has_odd_backslash_escape_bytes(text.as_bytes(), start) {
        return None;
    }
    let remaining = text.get(start..)?;
    let relative_end = remaining.find('>')?;
    let end = start + relative_end + '>'.len_utf8();
    let content = remaining.get('<'.len_utf8()..relative_end)?;
    is_uri_autolink(content)
        .then_some(start..end)
        .or_else(|| is_email_autolink(content).then_some(start..end))
}

/// Applies the Markdown URI-autolink grammar without decoding or rewriting its payload.
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

/// Applies the conservative e-mail-autolink grammar used for protection.
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

/// Reports whether an autolink character is safe to keep inside an angle span.
fn is_autolink_character(character: char) -> bool {
    !character.is_whitespace() && !character.is_control() && !matches!(character, '<' | '>')
}

/// Finds whitespace-delimited URL and path tokens that contain an ellipsis.
fn semantic_token_spans(text: &str) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut token_start = None;
    for (index, character) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        if character.is_whitespace() {
            if let Some(start) = token_start.take()
                && text.get(start..index).is_some_and(is_semantic_token)
            {
                spans.push(start..index);
            }
        } else if token_start.is_none() {
            token_start = Some(index);
        }
    }
    spans
}

/// Classifies a token as a bare URL or filesystem path requiring byte preservation.
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
    if let Some(token_kind) = kind
        && tracing::enabled!(Level::TRACE)
    {
        trace!(
            token_length = token.chars().count(),
            kind = token_kind,
            "protected semantic ellipsis token"
        );
    }
    kind.is_some()
}

/// Checks URL prefixes after removing one layer of punctuation around links.
fn looks_like_bare_url(token: &str) -> bool {
    let unwrapped = token.trim_start_matches(is_wrapper);
    is_uri_autolink(unwrapped) || unwrapped.starts_with("www.")
}

/// Checks Unix, home-relative, and Windows-drive path prefixes.
fn looks_like_path(token: &str) -> bool {
    let unwrapped = token.trim_start_matches(is_wrapper);
    unwrapped.starts_with('/')
        || unwrapped.starts_with("./")
        || unwrapped.starts_with("../")
        || unwrapped.starts_with("~/")
        || is_windows_drive_path(unwrapped)
}

/// Identifies punctuation that can wrap a URL or path without belonging to it.
const fn is_wrapper(character: char) -> bool { matches!(character, '(' | '[' | '{' | '"' | '\'') }

/// Recognises a drive-letter path while requiring a slash after the colon.
fn is_windows_drive_path(token: &str) -> bool {
    matches!(token.as_bytes(), [drive, b':', b'/' | b'\\', ..] if drive.is_ascii_alphabetic())
}

/// Sorts and merges overlapping protected ranges before prose replacement walks them.
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
    // Wrapper over `tracing_test::traced_test`; see `test_macros` for why.
    use test_macros::traced_test;

    use super::*;

    #[rstest::rstest]
    #[case::prose("wait...", Vec::<&str>::new())]
    #[case::link("[wait...](target)", vec!["[wait...](target)"])]
    #[case::url("see https://example.com/a...b", vec!["https://example.com/a...b"])]
    #[case::escaped_autolink(r"\<https://example.com/a...b>", Vec::<&str>::new())]
    #[case::path("open ./a/.../b next", vec!["./a/.../b"])]
    #[case::unicode("é <https://example.com/a...b> 例", vec!["<https://example.com/a...b>"])]
    #[case::windows_path(r"open C:\a\...\b next", vec![r"C:\a\...\b"])]
    #[case::slash_prose("choose and/or... input/output...", Vec::<&str>::new())]
    fn finds_literal_spans(#[case] input: &str, #[case] expected: Vec<&str>) {
        let actual = literal_spans(input)
            .into_iter()
            .map(|span| {
                input
                    .get(span)
                    .expect("protected span must have valid UTF-8 boundaries")
            })
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
                prop_assert!(matches!(pair, [previous, next] if previous.end < next.start));
            }
        }
    }
}
