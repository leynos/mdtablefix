//! Span-grouping helpers for inline token streams.
//!
//! These functions extend grouped spans over punctuation, whitespace, adjacent
//! footnote markers, and chained inline code or link tokens during
//! `determine_token_span`.
//! The module also provides `try_match_date_sequence`, which recognizes
//! contiguous day–month–year token runs and groups them into a single atomic
//! span before `determine_token_span` performs the standard punctuation and
//! link grouping pass.

use tracing::debug;
use unicode_width::UnicodeWidthStr;

use super::predicates::{
    is_inline_code_token,
    is_month_name,
    is_numeric_day,
    is_opening_punct,
    is_ordinal_day,
    is_trailing_punct,
    is_trailing_punctuation_token,
    is_whitespace_token,
    is_year,
    looks_like_footnote_ref,
    looks_like_link,
};

/// Marks how a grouped token span should behave during wrapping.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in crate::wrap::inline) enum SpanKind {
    /// Treat the span as ordinary prose.
    General,
    /// Treat the span as an inline code sequence.
    Code,
    /// Treat the span as a Markdown link or image link.
    Link,
    /// Treat the span as a GitHub Flavoured Markdown footnote reference.
    FootnoteRef,
}

/// Extends a grouped span over trailing punctuation tokens and updates `width`.
pub(in crate::wrap::inline) fn extend_punctuation(
    tokens: &[String],
    mut j: usize,
    width: &mut usize,
) -> usize {
    while j < tokens.len() && is_trailing_punctuation_token(&tokens[j]) {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
    }
    j
}

/// Returns the exclusive end of a date-like token run beginning at `start`.
#[tracing::instrument(level = "trace", skip(tokens), ret)]
pub(in crate::wrap::inline) fn try_match_date_sequence(
    tokens: &[String],
    start: usize,
) -> Option<usize> {
    if let Some(end) = match_ordinal_day_month_year(tokens, start) {
        debug!(
            start,
            end,
            pattern = "ordinal_day_month_year",
            "matched date sequence"
        );
        Some(end)
    } else if let Some(end) = match_numeric_day_month_year(tokens, start) {
        debug!(
            start,
            end,
            pattern = "numeric_day_month_year",
            "matched date sequence"
        );
        Some(end)
    } else if let Some(end) = match_month_numeric_day_year(tokens, start) {
        debug!(
            start,
            end,
            pattern = "month_numeric_day_year",
            "matched date sequence"
        );
        Some(end)
    } else {
        None
    }
}

#[tracing::instrument(level = "trace", skip(tokens), ret)]
pub(in crate::wrap::inline) fn date_token_span(
    tokens: &[String],
    start: usize,
) -> Option<(usize, usize)> {
    let date_end = try_match_date_sequence(tokens, start)?;
    let mut date_width = tokens[start..date_end]
        .iter()
        .map(|token| UnicodeWidthStr::width(token.as_str()))
        .sum();
    if let Some((_, footnote_end)) =
        try_couple_footnote_reference(tokens, date_end, SpanKind::General, &mut date_width)
    {
        return Some((footnote_end, date_width));
    }
    Some((date_end, date_width))
}

fn match_ordinal_day_month_year(tokens: &[String], start: usize) -> Option<usize> {
    let tokens = extract_five(tokens, start)?;
    match_pattern(tokens, is_ordinal_day, is_whitespace_token, is_month_name).then_some(start + 5)
}

fn match_numeric_day_month_year(tokens: &[String], start: usize) -> Option<usize> {
    let tokens = extract_five(tokens, start)?;
    match_pattern(tokens, is_numeric_day, is_whitespace_token, is_month_name).then_some(start + 5)
}

fn match_month_numeric_day_year(tokens: &[String], start: usize) -> Option<usize> {
    let tokens = extract_five(tokens, start)?;
    match_pattern(tokens, is_month_name, is_whitespace_token, is_numeric_day).then_some(start + 5)
}

#[derive(Clone, Copy)]
struct FiveTokens<'a> {
    first: &'a str,
    space1: &'a str,
    second: &'a str,
    space2: &'a str,
    year: &'a str,
}

fn extract_five(tokens: &[String], start: usize) -> Option<FiveTokens<'_>> {
    Some(FiveTokens {
        first: tokens.get(start)?.as_str(),
        space1: tokens.get(start + 1)?.as_str(),
        second: tokens.get(start + 2)?.as_str(),
        space2: tokens.get(start + 3)?.as_str(),
        year: tokens.get(start + 4)?.as_str(),
    })
}

fn match_pattern<F1, F2, F3>(
    tokens: FiveTokens<'_>,
    first_matches: F1,
    separator_matches: F2,
    second_matches: F3,
) -> bool
where
    F1: Fn(&str) -> bool,
    F2: Fn(&str) -> bool,
    F3: Fn(&str) -> bool,
{
    first_matches(tokens.first)
        && separator_matches(tokens.space1)
        && second_matches(tokens.second)
        && separator_matches(tokens.space2)
        && is_year(tokens.year)
}
/// Decide whether whitespace between grouped tokens should stay attached to the
/// current span.
pub(in crate::wrap::inline) fn should_couple_whitespace(
    kind: SpanKind,
    next_token: Option<&String>,
    following_token: Option<&String>,
) -> bool {
    match (kind, next_token, following_token) {
        (SpanKind::Link, Some(next), _)
            if looks_like_link(next)
                || is_inline_code_token(next)
                || is_trailing_punctuation_token(next) =>
        {
            true
        }
        (SpanKind::Code, Some(next), _) if is_trailing_punctuation_token(next) => true,
        (SpanKind::General, Some(next), Some(following))
            if looks_like_footnote_ref(next) && following == ":" =>
        {
            true
        }
        _ => false,
    }
}

/// Merges a backtick-opened code span into one grouped span and updates
/// `width`.
#[inline]
pub(in crate::wrap::inline) fn merge_code_span(
    tokens: &[String],
    i: usize,
    width: &mut usize,
) -> usize {
    debug_assert!(
        tokens[i] == "`",
        "merge_code_span requires a single backtick opener"
    );
    let mut j = i + 1;
    while j < tokens.len() && tokens[j] != "`" {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
    }
    if j < tokens.len() {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
        j = extend_punctuation(tokens, j, width);
    }
    j
}

/// Extends `end` by one token and any trailing punctuation that follows it.
pub(in crate::wrap::inline) fn absorb_token_and_trailing_punctuation(
    tokens: &[String],
    end: usize,
    width: &mut usize,
) -> usize {
    *width += UnicodeWidthStr::width(tokens[end].as_str());
    extend_punctuation(tokens, end + 1, width)
}

/// Couples an opener-followed inline link, such as `([1](url))`, into the
/// current span.
pub(in crate::wrap::inline) fn try_couple_inline_link_after_opener(
    tokens: &[String],
    end: usize,
    width: &mut usize,
) -> Option<(SpanKind, usize)> {
    let opener = tokens.get(end)?;
    let link = tokens.get(end + 1)?;
    if !opener.chars().all(is_opening_punct) || !looks_like_link(link) {
        return None;
    }

    *width += UnicodeWidthStr::width(opener.as_str());
    *width += UnicodeWidthStr::width(link.as_str());
    Some((SpanKind::Link, extend_punctuation(tokens, end + 2, width)))
}

/// Couples an adjacent footnote reference into the current span when appropriate.
pub(in crate::wrap::inline) fn try_couple_footnote_reference(
    tokens: &[String],
    end: usize,
    kind: SpanKind,
    width: &mut usize,
) -> Option<(SpanKind, usize)> {
    let token = tokens.get(end)?;
    if !looks_like_footnote_ref(token) {
        return None;
    }

    match kind {
        SpanKind::General => {
            let previous = end
                .checked_sub(1)
                .and_then(|previous| tokens.get(previous))?;
            let follows_punctuation = previous.chars().last().is_some_and(is_trailing_punct);
            let follows_space_before_colon = previous.chars().all(char::is_whitespace)
                && tokens.get(end + 1).is_some_and(|token| token == ":");
            if !follows_punctuation && !follows_space_before_colon {
                return None;
            }
            Some((
                SpanKind::FootnoteRef,
                absorb_token_and_trailing_punctuation(tokens, end, width),
            ))
        }
        SpanKind::Code | SpanKind::Link => Some((
            kind,
            absorb_token_and_trailing_punctuation(tokens, end, width),
        )),
        SpanKind::FootnoteRef => None,
    }
}

#[cfg(test)]
mod span_helper_props {
    //! Property tests for inline span helper date matching.

    use proptest::prelude::*;

    use super::try_match_date_sequence;
    use crate::wrap::inline::date_strategies::date_sequence_tokens_strategy;

    fn prefixed_date_sequence_tokens_strategy() -> BoxedStrategy<Vec<String>> {
        (
            date_sequence_tokens_strategy(),
            prop_oneof![Just('('), Just('['), Just('"')],
        )
            .prop_map(|(mut tokens, opener)| {
                tokens[0].insert(0, opener);
                tokens
            })
            .boxed()
    }

    fn non_whitespace_separator_strategy() -> BoxedStrategy<String> {
        prop_oneof![Just("-"), Just("_"), Just("/"), Just(","), Just(".")]
            .prop_map(str::to_string)
            .boxed()
    }

    #[test]
    fn prop_try_match_date_sequence_rejects_empty_slice() {
        assert!(try_match_date_sequence(&[], 0).is_none());
    }

    proptest! {
        #[test]
        fn prop_try_match_date_sequence_accepts_all_valid_patterns(
            tokens in date_sequence_tokens_strategy(),
        ) {
            prop_assert_eq!(try_match_date_sequence(&tokens, 0), Some(5));
        }

        #[test]
        fn prop_try_match_date_sequence_accepts_leading_opener_on_first_component(
            tokens in prefixed_date_sequence_tokens_strategy(),
        ) {
            prop_assert_eq!(try_match_date_sequence(&tokens, 0), Some(5));
        }

        #[test]
        fn prop_try_match_date_sequence_span_end_equals_start_plus_five(
            (date_tokens, offset) in (date_sequence_tokens_strategy(), 0usize..=8usize),
        ) {
            let mut tokens = vec!["filler".to_string(); offset];
            tokens.extend(date_tokens);
            prop_assert_eq!(try_match_date_sequence(&tokens, offset), Some(offset + 5));
        }

        #[test]
        fn prop_try_match_date_sequence_rejects_two_part(
            mut tokens in date_sequence_tokens_strategy(),
        ) {
            tokens.truncate(3);
            prop_assert!(try_match_date_sequence(&tokens, 0).is_none());
        }

        #[test]
        fn prop_try_match_date_sequence_rejects_non_whitespace_separator(
            (mut tokens, separator) in (
                date_sequence_tokens_strategy(),
                non_whitespace_separator_strategy(),
            ),
        ) {
            tokens[1] = separator;
            prop_assert!(try_match_date_sequence(&tokens, 0).is_none());
        }
    }
}

#[cfg(test)]
mod tracing_tests {
    //! Traced-event tests for inline span helper instrumentation.

    use tracing_test::traced_test;

    use super::{date_token_span, try_match_date_sequence};

    fn date_tokens() -> [String; 5] {
        [
            "25th".to_string(),
            " ".to_string(),
            "December".to_string(),
            " ".to_string(),
            "2025".to_string(),
        ]
    }

    #[traced_test]
    #[test]
    fn try_match_date_sequence_emits_trace_event() {
        let tokens = date_tokens();

        let _ = try_match_date_sequence(&tokens, 0);

        assert!(logs_contain("try_match_date_sequence"));
    }

    #[traced_test]
    #[test]
    fn try_match_date_sequence_logs_matched_pattern() {
        let tokens = date_tokens();

        let _ = try_match_date_sequence(&tokens, 0);

        assert!(logs_contain("ordinal_day_month_year"));
    }

    #[traced_test]
    #[test]
    fn date_token_span_emits_trace_event() {
        let tokens = date_tokens();

        let _ = date_token_span(&tokens, 0);

        assert!(logs_contain("date_token_span"));
    }
}
