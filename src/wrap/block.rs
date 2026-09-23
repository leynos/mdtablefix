//! Block-level Markdown prefix classification shared by wrapping and table detection.
//!
//! The regex helpers centralise detection for headings, lists, blockquotes, footnotes,
//! markdownlint directives, and digit-prefixed paragraphs so wrapping and table handlers
//! stay in sync.

use regex::Regex;
use tracing::trace;

use crate::classify::{ClassifyCtx, LineClass, classify_line};

/// Returns the indentation width (treating tabs as four columns) and the byte
/// offset of the first non-space or tab character.
pub(crate) fn leading_indent(line: &str) -> (usize, usize) {
    let mut width = 0;
    let mut bytes = 0;
    for &b in line.as_bytes() {
        match b {
            b' ' => {
                width += 1;
                bytes += 1;
            }
            0x09 => {
                width += 4;
                bytes += 1;
            }
            _ => break,
        }
    }
    (width, bytes)
}

/// Matches bullet and ordered list prefixes captured for wrapping and table detection.
pub(super) static BULLET_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(\s*(?:[-*+]|\d+[.)])\s+(?:\[\s*(?:[xX]|\s)\s*\]\s*)?)(.*)",
    "bullet pattern regex should compile",
);

/// Matches footnote definition prefixes so they remain atomic during wrapping and table parsing.
pub(super) static FOOTNOTE_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(\s*)(\[\^[^]]+\]:\s*)(.*)$",
    "footnote pattern regex should compile",
);

/// Matches blockquote prefixes, capturing the marker run and the remainder for reuse.
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
pub(super) static MARKDOWNLINT_DIRECTIVE_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"(?i)^\s*<!--\s*markdownlint-(?:disable|enable|disable-line|disable-next-line)(?:\s+[A-Za-z0-9_\-/]+)*\s*-->\s*$",
    "markdownlint directive regex should compile",
);

/// Describes the Markdown block prefix detected by [`classify_block`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockKind {
    /// Lines that begin with `#`, `##`, and similar heading prefixes.
    Heading,
    /// Thematic breaks recognised by [`crate::classify::classify_line`].
    ///
    /// This covers `***`, `___`, `---`, spaced runs such as `- - -`, and the
    /// underscore run emitted by `--breaks`, none of which are table
    /// separators.
    ThematicBreak,
    /// Bullet or ordered list markers matched by [`BULLET_RE`].
    Bullet,
    /// Lines that begin with one or more `>` markers.
    Blockquote,
    /// Footnote definitions recognised by [`FOOTNOTE_RE`].
    FootnoteDefinition,
    /// Link reference definitions recognised by [`super::link_reference::LinkReferenceMatcher`].
    LinkReferenceDefinition,
    /// HTML-style markdownlint directives recognised by [`is_markdownlint_directive`].
    MarkdownlintDirective,
    /// Lines whose first non-whitespace character is an ASCII digit.
    DigitPrefix,
}

/// Classifies block-level Markdown prefixes shared by wrapping and table detection.
///
/// Structural roles shared with other passes come from [`classify_line`].
///
/// This function keeps only the residual block roles that `LineClass` does not
/// represent: blockquotes, footnotes, link definitions, and markdownlint
/// directives.  The shared classifier gives headings, thematic breaks, and
/// list items one precedence everywhere that needs them.
/// For example, passing "> quote" returns `Some(BlockKind::Blockquote)` while
/// "| cell |" yields `None` because the line is part of a table.
pub(crate) fn classify_block(
    line: &str,
    link_matcher: super::link_reference::LinkReferenceMatcher,
) -> Option<BlockKind> {
    let (indent_width, indent_bytes) = leading_indent(line);
    let trimmed = line[indent_bytes..].trim_start();

    match classify_line(line, &ClassifyCtx::default()) {
        LineClass::AtxHeading => return Some(BlockKind::Heading),
        LineClass::ThematicBreak => {
            trace!(
                indent_width,
                line_len = line.len(),
                "classifying a line as a thematic break"
            );
            return Some(BlockKind::ThematicBreak);
        }
        LineClass::ListItem => return Some(BlockKind::Bullet),
        _ => {}
    }
    if let Some(kind) = classify_residual_block(line, link_matcher) {
        return Some(kind);
    }
    if indent_width < 4 && trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Some(BlockKind::DigitPrefix);
    }
    None
}

/// Recognizes block starts that the shared line classifier does not represent.
///
/// Wrapping and Setext conversion call this only after obtaining their
/// structural decision from the production line classifier. It is the sole
/// boundary for regex and link-reference checks that remain outside that
/// classifier.
pub(crate) fn classify_residual_block(
    line: &str,
    link_matcher: super::link_reference::LinkReferenceMatcher,
) -> Option<BlockKind> {
    if leading_indent(line).0 >= 4 {
        return None;
    }
    if BLOCKQUOTE_RE.is_match(line) {
        return Some(BlockKind::Blockquote);
    }
    if FOOTNOTE_RE.is_match(line) {
        return Some(BlockKind::FootnoteDefinition);
    }
    if link_matcher.is_definition(line) || link_matcher.is_bare_label_only(line) {
        return Some(BlockKind::LinkReferenceDefinition);
    }
    if is_markdownlint_directive(line) {
        return Some(BlockKind::MarkdownlintDirective);
    }
    None
}

/// Recognise a Markdownlint control comment before wrapping can alter it.
///
/// Directives configure validation rather than prose layout, so classifying
/// them as blocks keeps their spelling and placement verbatim.
pub(super) fn is_markdownlint_directive(line: &str) -> bool {
    MARKDOWNLINT_DIRECTIVE_RE.is_match(line)
}

#[cfg(test)]
mod tests {
    //! Unit tests for block classification.

    use rstest::rstest;

    use super::*;
    use crate::wrap::LinkReferenceMatcher;

    #[rstest(
        line,
        expected,
        case("# Heading", Some(BlockKind::Heading)),
        case("   # Heading", Some(BlockKind::Heading)),
        case("    # Code block", None),
        case("	# Heading", None),
        case("---", Some(BlockKind::ThematicBreak)),
        case("***", Some(BlockKind::ThematicBreak)),
        case("___", Some(BlockKind::ThematicBreak)),
        case("   ---", Some(BlockKind::ThematicBreak)),
        case("- - -", Some(BlockKind::ThematicBreak)),
        case("* * *", Some(BlockKind::ThematicBreak)),
        case("    ---", None),
        case("--", None),
        case("- item", Some(BlockKind::Bullet)),
        case("1. item", Some(BlockKind::Bullet)),
        case("> quote", Some(BlockKind::Blockquote)),
        case("[^1]: footnote", Some(BlockKind::FootnoteDefinition)),
        case(
            "[ansible]: <https://docs.ansible.com/>",
            Some(BlockKind::LinkReferenceDefinition)
        ),
        case(
            "[label]: https://example.com",
            Some(BlockKind::LinkReferenceDefinition)
        ),
        case(
            "[label]: https://example.com \"Optional title\"",
            Some(BlockKind::LinkReferenceDefinition)
        ),
        case("[label]:", Some(BlockKind::LinkReferenceDefinition)),
        case("  [label]:", Some(BlockKind::LinkReferenceDefinition)),
        case("    [label]: https://example.com", None),
        case("    [label]:", None),
        case(
            "<!-- markdownlint-disable -->",
            Some(BlockKind::MarkdownlintDirective)
        ),
        case("2024 revenue", Some(BlockKind::DigitPrefix)),
        case("plain paragraph", None),
        case("| a | b |", None),
        case("#123", None),
        case("1) list", Some(BlockKind::Bullet)),
        case(" 2024", Some(BlockKind::DigitPrefix)),
        case("    1. code", None)
    )]
    fn classify_block_identifies_prefixes(line: &str, expected: Option<BlockKind>) {
        let matcher = LinkReferenceMatcher::production();
        assert_eq!(classify_block(line, matcher), expected);
    }

    #[rstest]
    #[case("<!-- markdownlint-disable -->", true)]
    #[case("<!-- markdownlint-disable-next-line MD013 -->", true)]
    #[case("<!-- markdownlint-enable -->", true)]
    #[case("<!-- markdownlint enable -->", false)]
    #[case("<!-- just a comment -->", false)]
    fn detects_markdownlint_directives(#[case] line: &str, #[case] expected: bool) {
        assert_eq!(is_markdownlint_directive(line), expected);
    }
}
