//! Shared parsing helpers for footnote processing.

use std::sync::LazyLock;

use regex::Regex;

pub(super) static FOOTNOTE_LINE_RE: LazyLock<Regex> = lazy_regex!(
    r"^(?P<indent>\s*)(?P<num>\d+)[.:]\s+(?P<rest>.*)$",
    "footnote line pattern should compile",
);

pub(super) static DEF_RE: LazyLock<Regex> = lazy_regex!(
    r"^(?P<prefix>(?:\s*>\s*)*\s*)\[\^(?P<num>\d+)\]\s*:(?P<rest>.*)$",
    "footnote definition pattern should compile",
);

#[derive(Clone, Copy)]
pub(super) struct DefinitionParts<'a> {
    pub(super) prefix: &'a str,
    pub(super) number: usize,
    pub(super) rest: &'a str,
}

/// Parses a footnote definition line into its constituent [`DefinitionParts`].
///
/// A definition matches [`DEF_RE`]: an optional blockquote prefix (`>`
/// markers with surrounding whitespace), a `[^N]:` marker bearing a decimal
/// footnote number, and the trailing text following the colon. The borrowed
/// `prefix` and `rest` fields point into `line`.
///
/// Returns [`None`] when `line` is not a definition, or when the captured
/// number does not parse as a [`usize`].
///
/// # Examples
///
/// ```text
/// "[^1]: See the note."  => Some { prefix: "", number: 1, rest: " See the note." }
/// "> > [^2]: Nested."     => Some { prefix: "> > ", number: 2, rest: " Nested." }
/// "    indented body"     => None  // no `[^N]:` marker
/// ```
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

/// Reports whether `line` continues the preceding footnote definition.
///
/// A continuation is any line whose first character is whitespace, marking
/// indented body text that belongs to the definition above it rather than
/// beginning a new construct. An empty `line` has no leading character and so
/// returns `false`.
///
/// # Examples
///
/// ```text
/// "    continued text" => true   // leading whitespace
/// "\tcontinued text"   => true   // leading tab
/// "[^1]: definition"   => false  // starts a new definition
/// ""                   => false  // no leading character
/// ```
#[inline]
pub(super) fn is_definition_continuation(line: &str) -> bool {
    line.chars().next().is_some_and(char::is_whitespace)
}
