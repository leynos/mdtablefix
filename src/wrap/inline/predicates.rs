//! Token and fragment predicates for inline Markdown wrapping.
//!
//! These helpers classify segmented tokens and rendered fragment text so span
//! grouping and post-wrap heuristics can recognise links, code, footnotes, and
//! punctuation without duplicating detection rules.
//! The module also provides date-component predicates for date-sequence
//! grouping: `is_month_name`, `is_ordinal_day`, `is_numeric_day`, and
//! `is_year`. These recognise English month names, ordinal and numeric day
//! tokens, and four-digit year tokens before wrapping.

pub(crate) use super::month_names::MONTH_NAMES;
use crate::wrap::observer::{Event, ObserverHandle};

/// Return whether `c` opens a punctuation wrapper around an atomic span.
///
/// Both ASCII delimiters and common Unicode opening quotes/brackets are
/// recognised so the wrapper can keep them with the link or code they open.
pub(in crate::wrap::inline) fn is_opening_punct(c: char) -> bool {
    matches!(c, '(' | '[' | '"') || "“‘（［【《「『".contains(c)
}

/// Return whether `c` closes or punctuates an atomic inline span.
///
/// This set intentionally includes Unicode sentence punctuation because those
/// characters must remain attached when a line ends beside a link or code.
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

/// Returns whether `token` is a full or abbreviated English month name.
pub(in crate::wrap::inline) fn is_month_name(token: &str) -> bool {
    let token = strip_leading_openers(token);
    month_names_for_len(token.len())
        .iter()
        .any(|month| token.eq_ignore_ascii_case(month))
}

/// Strips all leading opener punctuation characters from `token`.
fn strip_leading_openers(token: &str) -> &str {
    let mut rest = token;
    while let Some(ch) = rest.chars().next() {
        if is_opening_punct(ch) {
            rest = &rest[ch.len_utf8()..];
        } else {
            break;
        }
    }
    rest
}

/// Select the month-name table matching the token's byte length.
///
/// Length filtering avoids case-insensitive comparisons against every month;
/// callers then perform the actual spelling check on the returned slice.
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
    let token = strip_leading_openers(token);
    ["st", "nd", "rd", "th"]
        .iter()
        .find_map(|suffix| token.strip_suffix(suffix))
        .is_some_and(is_day_number)
}

/// Returns whether `token` is a numeric day number from 1 through 31.
pub(in crate::wrap::inline) fn is_numeric_day(token: &str) -> bool {
    let token = strip_leading_openers(token);
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

/// Parse a day token after its ordinal suffix has been removed.
///
/// The shared range check keeps ordinal and numeric day recognition aligned so
/// date grouping cannot accept an impossible day in one form only.
fn is_day_number(token: &str) -> bool { token.parse::<u8>().is_ok_and(is_day) }

/// Return whether a numeric day lies in the inclusive Common date range.
fn is_day(day: u8) -> bool { (1..=31).contains(&day) }

/// Returns whether `token` already looks like a complete Markdown link.
pub(in crate::wrap::inline) fn looks_like_link(token: &str) -> bool {
    (token.starts_with('[') || token.starts_with("!["))
        && token.contains("](")
        && token.ends_with(')')
}

/// Returns whether `token` looks like a complete GFM footnote reference.
pub(in crate::wrap::inline) fn looks_like_footnote_ref(
    token: &str,
    observer: &mut ObserverHandle<'_>,
) -> bool {
    let result = token
        .strip_prefix("[^")
        .and_then(|label| label.strip_suffix(']'))
        .is_some_and(|label| !label.is_empty());
    if let Some(observer) = observer.as_deref_mut() {
        observer.observe(Event::FootnoteRefChecked { token, result });
    }
    result
}

/// Returns whether `token` is a bare numeric bracket reference, or the closing
/// half of one.
///
/// The tokenizer emits a bracket without an inline destination as its own
/// token, so `[1]` reaches the wrapper as the two tokens `[` and `1]`: the
/// opener arrives alone and the reference is recognisable only from its closing
/// half. Rendered fragments carry the merged form instead, so both are accepted.
///
/// The digits end at the first closing bracket, so further closers there and
/// punctuation that follows them count as the trailing run, not the label.
///
/// Only ASCII digits are recognised. A short label such as `[a]` is ordinary
/// prose the wrapper may break at, and the `[^` of a footnote reference is a
/// separate case, so this predicate stays disjoint from `looks_like_link` and
/// `looks_like_footnote_ref`.
///
/// This predicate takes no observer, unlike [`looks_like_footnote_ref`]. Its
/// three callers are all speculative: two probe candidate tokens during span
/// grouping and one re-tests a trimmed variant while classifying a fragment, so
/// forwarding an observer would report branch attempts rather than an outcome.
/// The outcome is already reported once, as a `FragmentClassified` event
/// carrying `FragmentKind::BracketedRef`, the same way the footnote probes in
/// `classify_fragment` are handled.
pub(in crate::wrap::inline) fn looks_like_bracketed_reference(token: &str) -> bool {
    let label = token.strip_prefix('[').unwrap_or(token);
    let Some((digits, tail)) = label.split_once(']') else {
        return false;
    };

    !digits.is_empty()
        && digits.chars().all(|digit| digit.is_ascii_digit())
        && tail.chars().all(is_trailing_punct)
}

/// Returns whether `token` ends with an inline footnote reference.
pub(in crate::wrap::inline) fn ends_with_footnote_ref(
    token: &str,
    observer: &mut ObserverHandle<'_>,
) -> bool {
    let Some(start) = token.rfind("[^") else {
        return false;
    };

    looks_like_footnote_ref(&token[start..], observer)
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
#[path = "predicates_tracing_tests.rs"]
mod tracing_tests;

#[cfg(test)]
#[path = "predicates_tests.rs"]
mod tests;
