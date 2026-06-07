//! Span-grouping helpers for inline token streams.
//!
//! These functions extend grouped spans over punctuation, whitespace, adjacent
//! footnote markers, and chained inline code or link tokens during
//! `determine_token_span`.

use unicode_width::UnicodeWidthStr;

use super::predicates::{
    is_inline_code_token,
    is_opening_punct,
    is_trailing_punct,
    is_trailing_punctuation_token,
    looks_like_footnote_ref,
    looks_like_link,
    is_ordinal_day,
    is_month_name,
    is_year,
    is_numeric_day,
    is_whitespace_token,
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
pub(in crate::wrap::inline) fn try_match_date_sequence(
    tokens: &[String],
    start: usize,
) -> Option<usize> {
    match_ordinal_day_month_year(tokens, start)
        .or_else(|| match_numeric_day_month_year(tokens, start))
        .or_else(|| match_month_numeric_day_year(tokens, start))
}

fn match_ordinal_day_month_year(tokens: &[String], start: usize) -> Option<usize> {
    match_date_pattern(tokens, start, 0, 2, is_ordinal_day, is_month_name)
}

fn match_numeric_day_month_year(tokens: &[String], start: usize) -> Option<usize> {
    match_date_pattern(tokens, start, 0, 2, is_numeric_day, is_month_name)
}

fn match_month_numeric_day_year(tokens: &[String], start: usize) -> Option<usize> {
    match_date_pattern(tokens, start, 2, 0, is_numeric_day, is_month_name)
}

fn match_date_pattern(
    tokens: &[String],
    start: usize,
    day_offset: usize,
    month_offset: usize,
    is_day: fn(&str) -> bool,
    is_month: fn(&str) -> bool,
) -> Option<usize> {
    let day = tokens.get(start + day_offset)?;
    let month = tokens.get(start + month_offset)?;
    let space1 = tokens.get(start + 1)?;
    let space2 = tokens.get(start + 3)?;
    let year = tokens.get(start + 4)?;

    if is_day(day)
        && is_whitespace_token(space1)
        && is_month(month)
        && is_whitespace_token(space2)
        && is_year(year)
    {
        Some(start + 5)
    } else {
        None
    }
}
/// Decide whether whitespace between grouped tokens should stay attached to the
/// current span.
pub(in crate::wrap::inline) fn should_couple_whitespace(
    kind: SpanKind,
    next_token: Option<&String>,
) -> bool {
    match (kind, next_token) {
        (SpanKind::Link, Some(next))
            if looks_like_link(next)
                || is_inline_code_token(next)
                || is_trailing_punctuation_token(next) =>
        {
            true
        }
        (SpanKind::Code, Some(next)) if is_trailing_punctuation_token(next) => true,
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
            if !previous.chars().last().is_some_and(is_trailing_punct) {
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

mod span_helper_props {
    //! Property tests for inline span helper date matching.

    use proptest::prelude::*;

    use super::try_match_date_sequence;
    use crate::wrap::inline::predicates::MONTH_NAMES;

    fn month_name_strategy() -> BoxedStrategy<String> {
        prop::sample::select(&MONTH_NAMES)
            .prop_map(str::to_string)
            .boxed()
    }

    fn ordinal_day_strategy() -> BoxedStrategy<String> {
        (
            1u8..=31,
            prop_oneof![Just("st"), Just("nd"), Just("rd"), Just("th")],
        )
            .prop_map(|(day, suffix)| format!("{day}{suffix}"))
            .boxed()
    }

    fn numeric_day_strategy() -> BoxedStrategy<String> {
        (1u8..=31, any::<bool>())
            .prop_map(|(day, append_comma)| {
                if append_comma {
                    format!("{day},")
                } else {
                    day.to_string()
                }
            })
            .boxed()
    }

    fn year_strategy() -> BoxedStrategy<String> {
        (1000u16..=2999).prop_map(|year| year.to_string()).boxed()
    }

    fn date_sequence_tokens_strategy() -> BoxedStrategy<Vec<String>> {
        prop_oneof![
            (
                ordinal_day_strategy(),
                month_name_strategy(),
                year_strategy()
            )
                .prop_map(|(day, month, year)| vec![
                    day,
                    " ".into(),
                    month,
                    " ".into(),
                    year
                ]),
            (
                numeric_day_strategy(),
                month_name_strategy(),
                year_strategy()
            )
                .prop_map(|(day, month, year)| vec![
                    day,
                    " ".into(),
                    month,
                    " ".into(),
                    year
                ]),
            (
                month_name_strategy(),
                numeric_day_strategy(),
                year_strategy()
            )
                .prop_map(|(month, day, year)| vec![
                    month,
                    " ".into(),
                    day,
                    " ".into(),
                    year
                ]),
        ]
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
