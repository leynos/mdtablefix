//! Link reference definition matching and standalone title continuation state.
//!
//! [`LinkReferenceMatcher`] centralises regex access for link reference queries,
//! and [`LinkTitleWindow`] tracks whether the next line may be a standalone title.

use regex::Regex;

/// Matches link reference definition prefixes so they remain verbatim during wrapping.
///
/// The pattern does not handle balanced nested brackets or escaped brackets in
/// link labels (for example, `[label [nested]]` or `[\[escaped\]]`). That
/// limitation is acceptable for issue #292 and the current regression tests.
pub(super) static LINK_REF_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r#"^(\s*)(\[[^\]]+\]:\s*)(.+?)(?:\s+("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\((?:[^)\\]|\\.)*\)))?\s*$"#,
    "link reference definition regex should compile",
);

/// Matches a standalone link reference title continuation line.
///
/// Per `CommonMark` spec §4.7 the title may appear on the line immediately
/// following the URL and must be enclosed in `"…"`, `'…'`, or `(…)`,
/// with at most three leading spaces and only optional trailing whitespace.
pub(super) static LINK_TITLE_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r#"^\s{0,3}("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\((?:[^)\\]|\\.)*\))\s*$"#,
    "link reference standalone title regex should compile",
);

/// Injected regex pair for link reference definition queries.
#[derive(Debug, Clone, Copy)]
pub(super) struct LinkReferenceMatcher {
    link_ref: &'static Regex,
    link_title: &'static Regex,
}

impl LinkReferenceMatcher {
    /// Returns the production matcher backed by compiled workspace regexes.
    #[must_use]
    pub(super) fn production() -> Self {
        Self {
            link_ref: &LINK_REF_RE,
            link_title: &LINK_TITLE_RE,
        }
    }

    /// Returns `true` when `line` matches a link reference definition.
    #[must_use]
    pub(super) fn is_definition(&self, line: &str) -> bool { self.link_ref.is_match(line) }

    /// Returns whether a standalone title may follow on the next line.
    ///
    /// `None` means `line` is not a link reference definition. `Some(true)`
    /// means the definition has no inline title; `Some(false)` means a title
    /// is already present on the same line.
    #[must_use]
    pub(super) fn standalone_title_need(&self, line: &str) -> Option<bool> {
        let cap = self.link_ref.captures(line)?;
        Some(cap.get(4).is_none())
    }

    /// Returns `true` when `line` is a valid standalone link reference title.
    #[must_use]
    pub(super) fn is_standalone_title_line(&self, line: &str) -> bool {
        self.link_title.is_match(line)
    }
}

/// Outcome of observing one line while awaiting a standalone title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LinkTitleWindowOutcome {
    /// Emit the line verbatim and continue without reflow.
    EmitVerbatim,
    /// Close the window and reprocess the line through normal wrapping.
    Reprocess,
}

/// Tracks whether the line after a bare link reference may be a standalone title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum LinkTitleWindow {
    /// No standalone title continuation is expected.
    #[default]
    Closed,
    /// The previous line was a bare link reference definition.
    AwaitingStandaloneTitle,
}

impl LinkTitleWindow {
    /// Closes the window after leaving fenced code or other interrupting context.
    pub(super) fn observe_fence_context(&mut self) { *self = Self::Closed; }

    /// Opens the window after emitting a bare link reference definition.
    pub(super) fn observe_bare_definition(&mut self) { *self = Self::AwaitingStandaloneTitle; }

    /// Inspects the next line when a standalone title may follow.
    ///
    /// Returns `None` when the window is closed. Otherwise the window is
    /// always closed before the caller acts on the outcome.
    pub(super) fn observe_next_line(
        &mut self,
        line: &str,
        matcher: LinkReferenceMatcher,
    ) -> Option<LinkTitleWindowOutcome> {
        if *self != Self::AwaitingStandaloneTitle {
            return None;
        }

        *self = Self::Closed;
        if line.trim().is_empty() || matcher.is_standalone_title_line(line) {
            return Some(LinkTitleWindowOutcome::EmitVerbatim);
        }
        Some(LinkTitleWindowOutcome::Reprocess)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest(
        line,
        expected,
        case("[ansible]: <https://docs.ansible.com/>", Some(true)),
        case("[label]: https://example.com", Some(true)),
        case("[label]: path_(v1)", Some(true)),
        case("[label]: https://example.com \"Inline title\"", Some(false)),
        case("[label]: https://example.com 'Inline title'", Some(false)),
        case("[label]: https://example.com (Inline title)", Some(false)),
        case("plain prose", None)
    )]
    fn standalone_title_need_detects_bare_urls(line: &str, expected: Option<bool>) {
        let matcher = LinkReferenceMatcher::production();
        assert_eq!(matcher.standalone_title_need(line), expected);
    }

    #[rstest(
        line,
        expected,
        case("  \"a title\"", true),
        case("'a title'", true),
        case("(a title)", true),
        case("", false),
        case("plain prose", false),
        case("    \"indented title\"", false)
    )]
    fn is_standalone_title_line_detects_titles(line: &str, expected: bool) {
        let matcher = LinkReferenceMatcher::production();
        assert_eq!(matcher.is_standalone_title_line(line), expected);
    }

    #[test]
    fn window_emits_blank_line_verbatim() {
        let matcher = LinkReferenceMatcher::production();
        let mut window = LinkTitleWindow::AwaitingStandaloneTitle;
        assert_eq!(
            window.observe_next_line("", matcher),
            Some(LinkTitleWindowOutcome::EmitVerbatim)
        );
        assert_eq!(window, LinkTitleWindow::Closed);
    }

    #[test]
    fn window_reprocesses_non_title_prose() {
        let matcher = LinkReferenceMatcher::production();
        let mut window = LinkTitleWindow::AwaitingStandaloneTitle;
        assert_eq!(
            window.observe_next_line("Paragraph text here.", matcher),
            Some(LinkTitleWindowOutcome::Reprocess)
        );
        assert_eq!(window, LinkTitleWindow::Closed);
    }
}
