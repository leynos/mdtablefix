//! Span-grouping helpers for inline token streams.
//!
//! These functions extend grouped spans over punctuation, whitespace, adjacent
//! footnote markers, and chained inline code or link tokens during
//! `determine_token_span`.
//! The module also provides `try_match_date_sequence`, which recognizes
//! contiguous day–month–year token runs and groups them into a single atomic
//! span before `determine_token_span` performs the standard punctuation and
//! link grouping pass.

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
    looks_like_bracketed_reference,
    looks_like_footnote_ref,
    looks_like_link,
};
// `SpanKind` is domain vocabulary carried on `Event` values, so it lives in the
// observer port beside `FragmentKind`. Re-exported here so the grouping helpers
// and their callers keep using the familiar `span_helpers::SpanKind` path.
pub(in crate::wrap::inline) use crate::wrap::observer::SpanKind;
use crate::wrap::observer::{Event, ObserverHandle};

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
///
/// The matched pattern is reported through `observer` as a stable category
/// name, so callers can tell which of the three shapes was recognized without
/// the helper touching a logging vendor.
pub(in crate::wrap::inline) fn try_match_date_sequence(
    tokens: &[String],
    start: usize,
    observer: &mut ObserverHandle<'_>,
) -> Option<usize> {
    let (end, pattern) = match_date_pattern(tokens, start)?;
    if let Some(observer) = observer.as_deref_mut() {
        observer.observe(Event::DateSequenceMatched {
            start,
            end,
            pattern,
        });
    }
    Some(end)
}

/// The date shapes recognized, paired with the stable name each reports.
///
/// Order is the precedence: the first matcher to accept decides the span and
/// the reported pattern. It is significant rather than incidental, because the
/// shapes overlap on their leading tokens.
type DateMatcher = (fn(&[String], usize) -> Option<usize>, &'static str);

/// The ordered matcher table backing [`match_date_pattern`].
const DATE_MATCHERS: [DateMatcher; 3] = [
    (match_ordinal_day_month_year, "ordinal_day_month_year"),
    (match_numeric_day_month_year, "numeric_day_month_year"),
    (match_month_numeric_day_year, "month_numeric_day_year"),
];

/// Returns the exclusive end and stable pattern name of the first date shape
/// matching at `start`, or `None` when none of them does.
///
/// This is the pure predicate half of [`try_match_date_sequence`], separated so
/// the matcher precedence is one readable table rather than a branch chain, and
/// so the pattern name travels with the match instead of being attached by the
/// caller.
fn match_date_pattern(tokens: &[String], start: usize) -> Option<(usize, &'static str)> {
    DATE_MATCHERS
        .iter()
        .find_map(|(matcher, pattern)| matcher(tokens, start).map(|end| (end, *pattern)))
}

/// Return the first token span representing a complete date, optionally
/// coupled with its following footnote reference.
///
/// The width is calculated over every token in the date so the wrapping stage
/// treats the date as one indivisible display unit.
pub(in crate::wrap::inline) fn date_token_span(
    tokens: &[String],
    start: usize,
    observer: &mut ObserverHandle<'_>,
) -> Option<(usize, usize)> {
    let date_end = try_match_date_sequence(tokens, start, observer)?;
    let date_width = tokens[start..date_end]
        .iter()
        .map(|token| UnicodeWidthStr::width(token.as_str()))
        .sum();
    let mut cursor = SpanCursor {
        end: date_end,
        width: date_width,
        kind: SpanKind::General,
    };
    if try_couple_footnote_reference(tokens, &mut cursor, observer) {
        return Some((cursor.end, cursor.width));
    }
    Some((date_end, date_width))
}

/// Match an ordinal day, month name, and year separated by whitespace.
fn match_ordinal_day_month_year(tokens: &[String], start: usize) -> Option<usize> {
    let tokens = extract_five(tokens, start)?;
    match_pattern(tokens, is_ordinal_day, is_whitespace_token, is_month_name).then_some(start + 5)
}

/// Match a numeric day, month name, and year separated by whitespace.
fn match_numeric_day_month_year(tokens: &[String], start: usize) -> Option<usize> {
    let tokens = extract_five(tokens, start)?;
    match_pattern(tokens, is_numeric_day, is_whitespace_token, is_month_name).then_some(start + 5)
}

/// Match a month name, numeric day, and year separated by whitespace.
fn match_month_numeric_day_year(tokens: &[String], start: usize) -> Option<usize> {
    let tokens = extract_five(tokens, start)?;
    match_pattern(tokens, is_month_name, is_whitespace_token, is_numeric_day).then_some(start + 5)
}

/// Borrow the five tokens needed for a day/month/year pattern check.
#[derive(Clone, Copy)]
struct FiveTokens<'a> {
    /// The first date component.
    first: &'a str,
    /// Whitespace separating the first and second components.
    space1: &'a str,
    /// The second date component.
    second: &'a str,
    /// Whitespace separating the second component and year.
    space2: &'a str,
    /// The four-digit year candidate.
    year: &'a str,
}

/// Extract a contiguous five-token window without allocating or indexing past
/// the end of the stream.
fn extract_five(tokens: &[String], start: usize) -> Option<FiveTokens<'_>> {
    Some(FiveTokens {
        first: tokens.get(start)?.as_str(),
        space1: tokens.get(start + 1)?.as_str(),
        second: tokens.get(start + 2)?.as_str(),
        space2: tokens.get(start + 3)?.as_str(),
        year: tokens.get(start + 4)?.as_str(),
    })
}

/// Apply component predicates to a borrowed date window.
///
/// The separator predicate is used for both whitespace positions, which keeps
/// all accepted date forms consistent about requiring token boundaries.
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
    observer: &mut ObserverHandle<'_>,
) -> bool {
    match (kind, next_token, following_token) {
        (SpanKind::Link, Some(next), _) if couples_after_link(next) => true,
        (SpanKind::Code, Some(next), _) if is_trailing_punctuation_token(next) => true,
        (SpanKind::General, Some(next), Some(following))
            if is_colon_footnote(next, following, observer) =>
        {
            true
        }
        _ => false,
    }
}

/// Reports whether a token stays attached to a completed link span.
///
/// A following link, inline code span, or trailing punctuation continues the
/// same atomic unit; ordinary prose after the link starts a new one.
fn couples_after_link(next: &str) -> bool {
    looks_like_link(next) || is_inline_code_token(next) || is_trailing_punctuation_token(next)
}

/// Reports whether `following` is the colon that closes a footnote reference.
///
/// The reference probe takes the observer, so it runs only for the general kind
/// where that shape decides coupling.
fn is_colon_footnote(next: &str, following: &str, observer: &mut ObserverHandle<'_>) -> bool {
    looks_like_footnote_ref(next, observer) && following == ":"
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

/// The span under construction while grouping extends it.
///
/// Grouping is a sequence of small decisions that each move the exclusive end
/// forward, so the three values travel together rather than as separate
/// parameters.
pub(in crate::wrap::inline) struct SpanCursor {
    /// The exclusive end of the span so far.
    pub end: usize,
    /// The Unicode display width accumulated so far.
    pub width: usize,
    /// The classification of the span so far.
    pub kind: SpanKind,
}

/// Extends the cursor by one token and any trailing punctuation after it.
pub(in crate::wrap::inline) fn absorb_token_and_trailing_punctuation(
    tokens: &[String],
    cursor: &mut SpanCursor,
) {
    cursor.width += UnicodeWidthStr::width(tokens[cursor.end].as_str());
    cursor.end = extend_punctuation(tokens, cursor.end + 1, &mut cursor.width);
}

/// Couples an opener-followed inline link, such as `([1](url))`, into the
/// current span.
///
/// Returns whether the link was coupled.
pub(in crate::wrap::inline) fn try_couple_inline_link_after_opener(
    tokens: &[String],
    cursor: &mut SpanCursor,
) -> bool {
    let Some(opener) = tokens.get(cursor.end) else {
        return false;
    };
    let Some(link) = tokens.get(cursor.end + 1) else {
        return false;
    };
    if !opener.chars().all(is_opening_punct) || !looks_like_link(link) {
        return false;
    }

    cursor.width += UnicodeWidthStr::width(opener.as_str());
    cursor.width += UnicodeWidthStr::width(link.as_str());
    cursor.kind = SpanKind::Link;
    cursor.end = extend_punctuation(tokens, cursor.end + 2, &mut cursor.width);
    true
}

/// Couples a `[` opener to the numeric reference that closes it.
///
/// The tokenizer emits a bracket without an inline destination as its own
/// token, so `[1]` reaches this module as `[` followed by `1]`. Treated as two
/// independent wrap units, the opener fits at the end of a line while the
/// digits move to the next one, which splits the reference across the break and
/// leaves the formatter with an input it reflows differently on a second pass.
///
/// Returns whether the reference was coupled.
pub(in crate::wrap::inline) fn try_couple_bracketed_reference(
    tokens: &[String],
    cursor: &mut SpanCursor,
) -> bool {
    let Some(opener) = tokens.get(cursor.end) else {
        return false;
    };
    let Some(reference) = tokens.get(cursor.end + 1) else {
        return false;
    };
    if opener != "[" || !looks_like_bracketed_reference(reference) {
        return false;
    }

    cursor.width += UnicodeWidthStr::width(opener.as_str());
    cursor.width += UnicodeWidthStr::width(reference.as_str());
    cursor.kind = SpanKind::BracketedRef;
    cursor.end = extend_punctuation(tokens, cursor.end + 2, &mut cursor.width);
    true
}

/// Returns whether a general span should absorb the footnote reference at
/// `end`.
///
/// A reference binds to prose only when it follows punctuation, or when it
/// follows whitespace and precedes a colon.
fn general_span_accepts_footnote(tokens: &[String], end: usize) -> bool {
    let Some(previous) = end.checked_sub(1).and_then(|previous| tokens.get(previous)) else {
        return false;
    };
    let follows_punctuation = previous.chars().last().is_some_and(is_trailing_punct);
    let follows_space_before_colon = previous.chars().all(char::is_whitespace)
        && tokens.get(end + 1).is_some_and(|token| token == ":");
    follows_punctuation || follows_space_before_colon
}

/// Couples an adjacent footnote reference into the current span when
/// appropriate.
///
/// Returns whether the reference was coupled.
pub(in crate::wrap::inline) fn try_couple_footnote_reference(
    tokens: &[String],
    cursor: &mut SpanCursor,
    observer: &mut ObserverHandle<'_>,
) -> bool {
    let Some(token) = tokens.get(cursor.end) else {
        return false;
    };
    if !looks_like_footnote_ref(token, observer) {
        return false;
    }

    match cursor.kind {
        SpanKind::General => {
            if !general_span_accepts_footnote(tokens, cursor.end) {
                return false;
            }
            cursor.kind = SpanKind::FootnoteRef;
            absorb_token_and_trailing_punctuation(tokens, cursor);
            true
        }
        SpanKind::Code | SpanKind::Link => {
            absorb_token_and_trailing_punctuation(tokens, cursor);
            true
        }
        // A reference marker binds only to the opener that introduces it, so a
        // second one starts a span of its own.
        SpanKind::FootnoteRef | SpanKind::BracketedRef => false,
    }
}

#[cfg(test)]
#[path = "span_helper_coupling_tests.rs"]
mod coupling_tests;

#[cfg(test)]
#[path = "span_helper_props.rs"]
mod span_helper_props;

#[cfg(test)]
#[path = "span_helper_tracing_tests.rs"]
mod span_helper_tracing_tests;
