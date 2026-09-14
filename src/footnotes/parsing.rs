//! Shared parsing helpers for footnote processing.

use std::sync::LazyLock;

use regex::Regex;

/// Parses ordered-list lines that are eligible for footnote promotion.
///
/// Indentation and the body separator are captured because conversion must
/// preserve both when the list item becomes a definition.
pub(super) static FOOTNOTE_LINE_RE: LazyLock<Regex> = lazy_regex!(
    r"^(?P<indent>\s*)(?P<num>\d+)[.:]\s+(?P<rest>.*)$",
    "footnote line pattern should compile",
);

/// Parses a GFM definition header, including blockquote prefixes.
///
/// The prefix is kept separate from the number so renumbering can change only
/// the identifier and leave the surrounding Markdown structure intact.
pub(super) static DEF_RE: LazyLock<Regex> = lazy_regex!(
    r"^(?P<prefix>(?:\s*>\s*)*\s*)\[\^(?P<num>\d+)\]\s*:(?P<rest>.*)$",
    "footnote definition pattern should compile",
);

/// Borrowed components of a footnote definition header.
///
/// Keeping the prefix and body as slices lets renumbering rewrite the numeric
/// marker without normalizing indentation, blockquotes, or body whitespace.
#[derive(Clone, Copy)]
pub(super) struct DefinitionParts<'a> {
    /// Indentation and blockquote markers preceding the definition marker.
    pub(super) prefix: &'a str,
    /// Numeric identifier captured from the definition marker.
    pub(super) number: usize,
    /// Definition body after the colon, retained for token-aware rewriting.
    pub(super) rest: &'a str,
}

/// Extracts the parts of a definition header without allocating its slices.
///
/// Invalid numeric identifiers are rejected so callers can rely on a valid
/// mapping key when assigning sequential numbers.
pub(super) fn parse_definition(line: &str) -> Option<DefinitionParts<'_>> {
    DEF_RE.captures(line).and_then(|caps| {
        let number = caps["num"].parse::<usize>().ok()?;
        Some(DefinitionParts {
            prefix: caps.name("prefix").map_or("", |m| m.as_str()),
            number,
            rest: caps.name("rest").map_or("", |m| m.as_str()),
        })
    })
}

/// Reports whether a line can continue an indented footnote definition.
///
/// Any leading whitespace qualifies; blank lines are handled separately by
/// block scanners because they can separate adjacent definitions.
#[inline]
pub(super) fn is_definition_continuation(line: &str) -> bool {
    line.chars().next().is_some_and(char::is_whitespace)
}
