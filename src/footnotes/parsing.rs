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
/// `line` is a definition header when it matches [`DEF_RE`]: an optional
/// blockquote prefix (`>` markers with surrounding whitespace), a `[^N]:`
/// marker bearing a decimal footnote number, and the body following the colon.
/// The returned [`DefinitionParts`] borrows `prefix` and `rest` from `line`.
///
/// Returns [`None`] when `line` is not a definition header, or when the
/// captured number does not parse as a [`usize`]. Rejecting invalid numeric
/// identifiers lets callers rely on a valid mapping key when assigning
/// sequential numbers.
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

/// Reports whether `line` can continue an indented footnote definition.
///
/// Returns `true` when the first character of `line` is whitespace, marking
/// indented body text that belongs to the definition above it. Whitespace-only
/// lines therefore qualify as continuations too. Only an empty `line`, which
/// has no leading character, returns `false`; block scanners handle those empty
/// lines separately because they can separate adjacent definitions.
///
/// # Examples
///
/// ```text
/// "    continued text" => true   // leading whitespace
/// "\tcontinued text"   => true   // leading tab
/// "   "                => true   // whitespace-only
/// "[^1]: definition"   => false  // starts a new definition
/// ""                   => false  // no leading character
/// ```
#[inline]
pub(super) fn is_definition_continuation(line: &str) -> bool {
    line.chars().next().is_some_and(char::is_whitespace)
}
