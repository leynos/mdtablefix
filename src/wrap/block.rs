//! Block-level Markdown prefix classification shared by wrapping and table detection.
//!
//! The regex helpers centralise detection for headings, lists, blockquotes, footnotes,
//! markdownlint directives, and digit-prefixed paragraphs so wrapping and table handlers
//! stay in sync.

use regex::Regex;

pub(super) static BULLET_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(\s*(?:[-*+]|\d+[.)])\s+(?:\[\s*(?:[xX]|\s)\s*\]\s*)?)(.*)",
    "bullet pattern regex should compile",
);

pub(super) static FOOTNOTE_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(\s*)(\[\^[^]]+\]:\s*)(.*)$",
    "footnote pattern regex should compile",
);

pub(super) static BLOCKQUOTE_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(\s*(?:>\s*)+)(.*)$",
    "blockquote pattern regex should compile",
);

/// Matches `markdownlint` comment directives.
///
/// The regex is case-insensitive and recognises these forms with optional rule
/// names (including plugin rules such as `MD013/line-length` or
/// `plugin/rule-name`):
/// - `<!-- markdownlint-disable -->`
/// - `<!-- markdownlint-enable -->`
/// - `<!-- markdownlint-disable-line MD001 MD005 -->`
/// - `<!-- markdownlint-disable-next-line MD001 MD005 -->`
pub(super) static MARKDOWNLINT_DIRECTIVE_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(
    || {
        Regex::new(
        r"(?i)^\s*<!--\s*markdownlint-(?:disable|enable|disable-line|disable-next-line)(?:\s+[A-Za-z0-9_\-/]+)*\s*-->\s*$",
    )
    .expect("valid markdownlint regex")
    },
);

/// Describes the Markdown block prefix detected by [`classify_block`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockKind {
    /// Lines that begin with `#`, `##`, and similar heading prefixes.
    Heading,
    /// Bullet or ordered list markers matched by [`BULLET_RE`].
    Bullet,
    /// Lines that begin with one or more `>` markers.
    Blockquote,
    /// Footnote definitions recognised by [`FOOTNOTE_RE`].
    FootnoteDefinition,
    /// HTML-style markdownlint directives recognised by [`is_markdownlint_directive`].
    MarkdownlintDirective,
    /// Lines whose first non-whitespace character is an ASCII digit.
    DigitPrefix,
}

/// Classifies block-level Markdown prefixes shared by wrapping and table detection.
///
/// Detection order determines precedence when a line could match multiple prefixes.
/// The current precedence is: heading, bullet, blockquote, footnote definition,
/// markdownlint directive, digit prefix. Headings outrank bullets and blockquotes,
/// so inputs such as "# 1" remain headings rather than list items. Headings ignore
/// indentation of four or more spaces so indented code remains untouched.
/// For example, passing "> quote" returns `Some(BlockKind::Blockquote)` while
/// "| cell |" yields `None` because the line is part of a table.
pub(crate) fn classify_block(line: &str) -> Option<BlockKind> {
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len());

    if indent < 4 && trimmed.starts_with('#') {
        return Some(BlockKind::Heading);
    }
    if BULLET_RE.is_match(line) {
        return Some(BlockKind::Bullet);
    }
    if BLOCKQUOTE_RE.is_match(line) {
        return Some(BlockKind::Blockquote);
    }
    if FOOTNOTE_RE.is_match(line) {
        return Some(BlockKind::FootnoteDefinition);
    }
    if is_markdownlint_directive(line) {
        return Some(BlockKind::MarkdownlintDirective);
    }
    if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Some(BlockKind::DigitPrefix);
    }
    None
}

#[inline]
pub(super) fn is_markdownlint_directive(line: &str) -> bool {
    MARKDOWNLINT_DIRECTIVE_RE.is_match(line)
}
