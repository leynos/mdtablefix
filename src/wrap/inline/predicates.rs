//! Token and fragment predicates for inline Markdown wrapping.
//!
//! These helpers classify segmented tokens and rendered fragment text so span
//! grouping and post-wrap heuristics can recognise links, code, footnotes, and
//! punctuation without duplicating detection rules.

pub(in crate::wrap::inline) fn is_opening_punct(c: char) -> bool {
    matches!(c, '(' | '[') || "（［【《「『".contains(c)
}

pub(in crate::wrap::inline) fn is_trailing_punct(c: char) -> bool {
    // ASCII closers + common Unicode closers and word-final punctuation
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '"' | '\''
    ) || "…—–»›）］】》」』、。，：；！？”.’".contains(c)
}

/// Returns whether `token` is a non-empty run of trailing punctuation.
///
/// The wrapper uses this to keep trailing punctuation attached to the
/// preceding link or code span during wrapping, rather than letting the
/// punctuation break onto the next line.
pub(in crate::wrap::inline) fn is_trailing_punctuation_token(token: &str) -> bool {
    !token.is_empty() && token.chars().all(is_trailing_punct)
}

/// Full and abbreviated English month names recognized in prose dates.
///
/// There are 23 entries: twelve full names plus eleven abbreviations, because
/// `May` is identical in both forms and is listed once. The entries are grouped
/// by byte length so `is_month_name` can avoid scanning impossible candidates.
pub(in crate::wrap::inline) const MONTH_NAMES: [&str; 23] = [
    "May",
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
    "June",
    "July",
    "March",
    "April",
    "August",
    "January",
    "October",
    "February",
    "November",
    "December",
    "September",
];

/// Returns whether `token` is a full or abbreviated English month name.
pub(in crate::wrap::inline) fn is_month_name(token: &str) -> bool {
    month_names_for_len(token.len())
        .iter()
        .any(|month| token.eq_ignore_ascii_case(month))
}

fn month_names_for_len(len: usize) -> &'static [&'static str] {
    match len {
        3 => &MONTH_NAMES[..12],
        4 => &MONTH_NAMES[12..14],
        5 => &MONTH_NAMES[14..16],
        6 => &MONTH_NAMES[16..17],
        7 => &MONTH_NAMES[17..19],
        8 => &MONTH_NAMES[19..22],
        9 => &MONTH_NAMES[22..],
        _ => &[],
    }
}

/// Returns whether `token` is an ordinal day number from 1st through 31st.
pub(in crate::wrap::inline) fn is_ordinal_day(token: &str) -> bool {
    ["st", "nd", "rd", "th"]
        .iter()
        .find_map(|suffix| token.strip_suffix(suffix))
        .is_some_and(is_day_number)
}

/// Returns whether `token` is a numeric day number from 1 through 31.
pub(in crate::wrap::inline) fn is_numeric_day(token: &str) -> bool {
    token
        .strip_suffix(',')
        .unwrap_or(token)
        .parse::<u8>()
        .is_ok_and(is_day)
}

/// Returns whether `token` is a year from 1000 through 2999, optionally
/// followed by trailing prose punctuation.
pub(in crate::wrap::inline) fn is_year(token: &str) -> bool {
    token
        .trim_end_matches(is_trailing_punct)
        .parse::<u16>()
        .is_ok_and(|year| (1000..=2999).contains(&year))
}

fn is_day_number(token: &str) -> bool { token.parse::<u8>().is_ok_and(is_day) }

fn is_day(day: u8) -> bool { (1..=31).contains(&day) }

/// Returns whether `token` already looks like a complete Markdown link.
pub(in crate::wrap::inline) fn looks_like_link(token: &str) -> bool {
    (token.starts_with('[') || token.starts_with("!["))
        && token.contains("](")
        && token.ends_with(')')
}

/// Returns whether `token` looks like a complete GFM footnote reference.
///
/// The `#[tracing::instrument]` attribute records the argument and return
/// value automatically.
#[tracing::instrument(level = "trace", ret)]
pub(in crate::wrap::inline) fn looks_like_footnote_ref(token: &str) -> bool {
    token
        .strip_prefix("[^")
        .and_then(|label| label.strip_suffix(']'))
        .is_some_and(|label| !label.is_empty())
}

/// Returns whether `token` ends with an inline footnote reference.
///
/// The `#[tracing::instrument]` attribute records the argument and return
/// value automatically.
#[tracing::instrument(level = "trace", ret)]
pub(in crate::wrap::inline) fn ends_with_footnote_ref(token: &str) -> bool {
    let Some(start) = token.rfind("[^") else {
        return false;
    };

    looks_like_footnote_ref(&token[start..])
}

/// Returns whether `token` contains only Unicode whitespace.
pub(in crate::wrap::inline) fn is_whitespace_token(token: &str) -> bool {
    token.chars().all(char::is_whitespace)
}

/// Returns whether `token` is a complete inline code span.
pub(in crate::wrap::inline) fn is_inline_code_token(token: &str) -> bool {
    token.len() > 1 && token.starts_with('`') && token.ends_with('`')
}

/// Returns whether `token` is a hyphen-terminated prefix that should bind to a
/// following inline code span (for example `pre-`, `LLM-`, or `(API-`).
///
/// Bare punctuation such as `-` or `---` is rejected so that ordinary dash
/// runs are not absorbed into the next atomic span. The alphabetic check uses
/// `char::is_alphabetic`, so Unicode-letter compounds (`pré-`, `naïve-`,
/// `字-`) are intentionally accepted alongside ASCII prefixes. Internal hyphen
/// chains (`state-of-the-art-`) are also accepted because such compounds
/// remain a single atomic wrap token by design.
///
/// The `#[tracing::instrument]` attribute records the argument and return
/// value as a TRACE-level event.
#[tracing::instrument(level = "trace", ret)]
pub(in crate::wrap::inline) fn ends_with_hyphen_prefix(token: &str) -> bool {
    token.ends_with('-') && token.chars().any(char::is_alphabetic)
}

/// Returns the substring beginning at the first Markdown link opener after any
/// leading opener punctuation.
pub(in crate::wrap::inline) fn link_text_after_leading_openers(text: &str) -> &str {
    let mut rest = text;
    while !rest.is_empty() {
        if rest.starts_with('[') || rest.starts_with("![") {
            return rest;
        }
        let Some(ch) = rest.chars().next() else {
            break;
        };
        if is_opening_punct(ch) {
            rest = &rest[ch.len_utf8()..];
        } else {
            break;
        }
    }
    rest
}

/// Strips one outer wrapper closing character from a link candidate when present.
fn strip_outer_link_wrapper_suffix(text: &str) -> Option<&str> {
    let last = text.chars().next_back()?;
    if matches!(last, ')' | ']' | '）' | '］' | '」' | '』' | '》') {
        Some(&text[..text.len() - last.len_utf8()])
    } else {
        None
    }
}

/// Returns whether rendered fragment text contains a Markdown link, including
/// links wrapped in outer opener punctuation.
pub(in crate::wrap::inline) fn fragment_is_link(text: &str) -> bool {
    if looks_like_link(text) {
        return true;
    }
    let mut candidate = link_text_after_leading_openers(text);
    while !candidate.is_empty() {
        if looks_like_link(candidate) {
            return true;
        }
        let Some(next) = strip_outer_link_wrapper_suffix(candidate) else {
            break;
        };
        candidate = next;
    }
    false
}

#[cfg(test)]
#[path = "predicate_date_props.rs"]
mod predicate_date_props;

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rstest::rstest;

    use super::{
        ends_with_hyphen_prefix,
        is_inline_code_token,
        is_opening_punct,
        is_trailing_punct,
        is_trailing_punctuation_token,
        is_whitespace_token,
        is_year,
        looks_like_footnote_ref,
    };

    fn backtick_run_strategy() -> BoxedStrategy<String> {
        prop::collection::vec(Just('`'), 1..8)
            .prop_map(|chars| chars.into_iter().collect::<String>())
            .boxed()
    }

    fn arbitrary_short_string_strategy() -> BoxedStrategy<String> {
        prop::collection::vec(any::<char>(), 0..24)
            .prop_map(|chars| chars.into_iter().collect::<String>())
            .boxed()
    }

    fn footnote_label_strategy() -> BoxedStrategy<String> {
        prop::string::string_regex("[a-zA-Z0-9_-]+")
            .expect("failed to build footnote label regex strategy")
            .boxed()
    }

    #[test]
    fn is_inline_code_token_rejects_lone_backtick_delimiter() {
        let delimiter = char::from(b'`');
        assert!(!is_inline_code_token(&delimiter.to_string()));
    }

    #[test]
    fn is_inline_code_token_accepts_complete_span() {
        let delimiter = char::from(b'`');
        let token = format!("{delimiter}code{delimiter}");
        assert!(is_inline_code_token(&token));
    }

    #[test]
    fn is_inline_code_token_matches_backtick_delimited_length_rule() {
        proptest!(|(token in backtick_run_strategy())| {
            let expected = token.len() > 1 && token.starts_with('`') && token.ends_with('`');
            prop_assert_eq!(is_inline_code_token(&token), expected);
        });
    }

    #[test]
    fn is_whitespace_token_matches_char_classification() {
        proptest!(|(token in arbitrary_short_string_strategy())| {
            prop_assert_eq!(
                is_whitespace_token(&token),
                token.chars().all(char::is_whitespace)
            );
        });
    }

    #[test]
    fn opening_and_trailing_punct_are_mutually_exclusive_for_ascii_letters() {
        for c in 'a'..='z' {
            assert!(!is_opening_punct(c));
            assert!(!is_trailing_punct(c));
        }
    }

    #[test]
    fn looks_like_footnote_ref_implies_non_empty_label() {
        proptest!(|(label in footnote_label_strategy())| {
            let token = format!("[^{label}]");
            prop_assert!(looks_like_footnote_ref(&token));
        });
    }

    #[test]
    fn looks_like_footnote_ref_rejects_empty_label() {
        assert!(!looks_like_footnote_ref("[^]"));
    }

    mod tracing_tests {
        //! Traced-event tests for predicate helpers.
        //!
        //! Verifies that `looks_like_footnote_ref` and
        //! `ends_with_footnote_ref` emit TRACE events when called,
        //! confirming that the `#[tracing::instrument]` attribute is
        //! effective at the declared log level.

        use tracing_test::traced_test;

        use super::super::{
            ends_with_footnote_ref,
            ends_with_hyphen_prefix,
            looks_like_footnote_ref,
        };

        #[traced_test]
        #[test]
        fn looks_like_footnote_ref_emits_trace_event() {
            let _ = looks_like_footnote_ref("[^1]");
            assert!(logs_contain("looks_like_footnote_ref"));
        }

        #[traced_test]
        #[test]
        fn ends_with_footnote_ref_emits_trace_event() {
            let _ = ends_with_footnote_ref("word.[^1]");
            assert!(logs_contain("ends_with_footnote_ref"));
        }

        #[traced_test]
        #[test]
        fn ends_with_hyphen_prefix_emits_trace_event() {
            let _ = ends_with_hyphen_prefix("pre-");
            assert!(logs_contain("ends_with_hyphen_prefix"));
        }
    }

    #[rstest]
    #[case("pre-", true)]
    #[case("LLM-", true)]
    #[case("(pre-", true)]
    #[case("pré-", true)]
    #[case("字-", true)]
    #[case("state-of-the-art-", true)]
    #[case("-", false)]
    #[case("---", false)]
    #[case("foo", false)]
    #[case("2024-", false)]
    fn ends_with_hyphen_prefix_classifies_tokens(#[case] token: &str, #[case] expected: bool) {
        assert_eq!(ends_with_hyphen_prefix(token), expected);
    }

    #[rstest]
    #[case(".", true)]
    #[case("!?", true)]
    #[case("...", true)]
    #[case("", false)]
    #[case("abc", false)]
    #[case(".x", false)]
    fn is_trailing_punctuation_token_classifies_tokens(
        #[case] token: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(is_trailing_punctuation_token(token), expected);
    }

    #[rstest]
    #[case("2025", true)]
    #[case("2025.", true)]
    #[case("2025,", true)]
    #[case("2008)", true)]
    #[case("2008).", true)]
    #[case("2008,)", true)]
    #[case("999", false)]
    #[case("3000", false)]
    #[case("2025th.", false)]
    #[case(".", false)]
    fn is_year_accepts_sentence_trailing_punctuation(#[case] token: &str, #[case] expected: bool) {
        assert_eq!(is_year(token), expected);
    }
}
