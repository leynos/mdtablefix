//! Link reference definition matching and standalone title continuation state.
//!
//! [`LinkReferenceMatcher`] centralizes regex access for link reference queries,
//! and [`LinkTitleWindow`] tracks whether the next line may be a standalone title.

use regex::Regex;

/// Matches link reference definition prefixes so they remain verbatim during wrapping.
///
/// The pattern does not handle balanced nested brackets or escaped brackets in
/// link labels (for example, `[label [nested]]` or `[\[escaped\]]`). That
/// limitation is acceptable for issue #292 and the current regression tests.
pub(super) static LINK_REF_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r#"^(\s*)(\[[^\]]+\]:\s*)(\S.*?)(?:\s+("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\((?:[^)\\]|\\.)*\)))?\s*$"#,
    "link reference definition regex should compile",
);

/// Matches a link reference label line whose destination continues on the next line.
pub(super) static BARE_LABEL_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(\s*)(\[[^\]]+\]:\s*)$",
    "bare link reference label regex should compile",
);

/// Matches an indented non-blank destination continuation line.
pub(super) static URL_CONTINUATION_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    concat!(
        r#"^\s+(?:<[^>\s]+>|\S+)(?:\s+("#,
        r#""(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\((?:[^)\\]|\\.)*\)"#,
        r#"))?\s*$"#,
    ),
    "link reference URL continuation regex should compile",
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

/// Matches an inline title suffix after a continued link reference destination.
pub(super) static INLINE_TITLE_SUFFIX_RE: std::sync::LazyLock<Regex> = lazy_regex!(
    r#"\s+("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\((?:[^)\\]|\\.)*\))\s*$"#,
    "link reference inline title suffix regex should compile",
);

/// Injected regex set for link reference definition queries.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LinkReferenceMatcher {
    link_ref: &'static Regex,
    bare_label: &'static Regex,
    url_continuation: &'static Regex,
    link_title: &'static Regex,
    inline_title_suffix: &'static Regex,
}

impl LinkReferenceMatcher {
    /// Returns the production matcher backed by compiled workspace regexes.
    #[must_use]
    pub(crate) fn production() -> Self {
        Self {
            link_ref: &LINK_REF_RE,
            bare_label: &BARE_LABEL_RE,
            url_continuation: &URL_CONTINUATION_RE,
            link_title: &LINK_TITLE_RE,
            inline_title_suffix: &INLINE_TITLE_SUFFIX_RE,
        }
    }

    /// Returns `true` when `line` matches a link reference definition.
    #[must_use]
    pub(super) fn is_definition(&self, line: &str) -> bool { self.link_ref.is_match(line) }

    /// Returns `true` when `line` is only a link reference label and colon.
    #[must_use]
    pub(super) fn is_bare_label_only(&self, line: &str) -> bool { self.bare_label.is_match(line) }

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

    /// Returns `true` when `line` is an indented URL continuation.
    #[must_use]
    pub(super) fn is_url_continuation_line(&self, line: &str) -> bool {
        self.url_continuation.is_match(line) && !is_markdown_prefixed_continuation(line)
    }

    fn url_continuation_has_inline_title(&self, line: &str) -> bool {
        self.inline_title_suffix.is_match(line)
    }
}

fn is_markdown_prefixed_continuation(line: &str) -> bool {
    let trimmed = line.trim_start();
    is_bullet_marker(trimmed)
        || is_ordered_list_marker(trimmed)
        || trimmed.starts_with('>')
        || trimmed.starts_with('#')
}

fn is_bullet_marker(trimmed: &str) -> bool {
    let mut chars = trimmed.chars();
    matches!(chars.next(), Some('-' | '*' | '+')) && chars.next().is_some_and(char::is_whitespace)
}

fn is_ordered_list_marker(trimmed: &str) -> bool {
    let mut end_idx = 0;
    for (idx, ch) in trimmed.char_indices() {
        if ch.is_ascii_digit() || matches!(ch, '.' | ')') {
            end_idx = idx + ch.len_utf8();
        } else {
            break;
        }
    }

    if end_idx == 0 || !matches!(trimmed.as_bytes()[end_idx - 1], b'.' | b')') {
        return false;
    }

    let number = &trimmed[..end_idx - 1];
    !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && trimmed[end_idx..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
}

/// Outcome of observing one line while awaiting a standalone title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkTitleWindowOutcome {
    /// Emit the line verbatim and continue without reflow.
    EmitVerbatim,
    /// Close the window and reprocess the line through normal wrapping.
    Reprocess,
}

/// Tracks whether a split link reference may have a destination or title continuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum LinkTitleWindow {
    /// No standalone title continuation is expected.
    #[default]
    Closed,
    /// The previous line was a bare link reference definition.
    AwaitingStandaloneTitle,
    /// The previous line was a link reference label whose destination is next.
    AwaitingUrlContinuation,
}

impl LinkTitleWindow {
    /// Closes the window after leaving fenced code or other interrupting context.
    pub(super) fn observe_fence_context(&mut self) { *self = Self::Closed; }

    /// Opens the window after emitting a bare link reference definition.
    pub(super) fn observe_bare_definition(&mut self) { *self = Self::AwaitingStandaloneTitle; }

    /// Opens the window after emitting a label-only link reference definition.
    pub(super) fn observe_bare_label(&mut self) { *self = Self::AwaitingUrlContinuation; }

    /// Inspects the next line when a standalone title may follow.
    ///
    /// Returns `None` when the window is closed. Otherwise the window is
    /// always closed before the caller acts on the outcome.
    pub(super) fn observe_next_line(
        &mut self,
        line: &str,
        matcher: LinkReferenceMatcher,
    ) -> Option<LinkTitleWindowOutcome> {
        match *self {
            Self::Closed => None,
            Self::AwaitingStandaloneTitle => {
                *self = Self::Closed;
                if line.trim().is_empty() || matcher.is_standalone_title_line(line) {
                    return Some(LinkTitleWindowOutcome::EmitVerbatim);
                }
                Some(LinkTitleWindowOutcome::Reprocess)
            }
            Self::AwaitingUrlContinuation => {
                if matcher.is_url_continuation_line(line) {
                    if matcher.url_continuation_has_inline_title(line) {
                        *self = Self::Closed;
                    } else {
                        *self = Self::AwaitingStandaloneTitle;
                    }
                    return Some(LinkTitleWindowOutcome::EmitVerbatim);
                }
                *self = Self::Closed;
                if line.trim().is_empty() {
                    return Some(LinkTitleWindowOutcome::EmitVerbatim);
                }
                Some(LinkTitleWindowOutcome::Reprocess)
            }
        }
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
