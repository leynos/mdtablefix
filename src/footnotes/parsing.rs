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

#[derive(Clone)]
pub(super) struct DefinitionParts<'a> {
    pub(super) prefix: &'a str,
    pub(super) number: usize,
    pub(super) rest: &'a str,
}

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

#[inline]
pub(super) fn is_definition_continuation(line: &str) -> bool {
    line.chars().next().is_some_and(char::is_whitespace)
}
