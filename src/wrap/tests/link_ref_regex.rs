//! Property-based tests for link reference regex correctness and title window state.

use proptest::prelude::*;

use crate::wrap::{
    BlockKind,
    classify_block,
    link_reference::{LinkReferenceMatcher, LinkTitleWindow, LinkTitleWindowOutcome},
};

/// Arbitrary label: one or more non-`]` printable ASCII characters.
fn arb_label() -> impl Strategy<Value = String> { "[A-Za-z0-9 _-]{1,40}".prop_map(|s| s) }

/// Arbitrary bare URL: starts with `https://` followed by alphanumerics.
fn arb_bare_url() -> impl Strategy<Value = String> {
    "[a-z]{3,8}://[A-Za-z0-9./-]{1,40}".prop_map(|s| s)
}

/// Arbitrary angle-bracketed URL destination.
fn arb_angle_url() -> impl Strategy<Value = String> {
    arb_bare_url().prop_map(|url| format!("<{url}>"))
}

/// Arbitrary inline title in double-quote form.
fn arb_dq_title() -> impl Strategy<Value = String> {
    "[A-Za-z0-9 ]{0,30}".prop_map(|s| format!("\"{s}\""))
}

/// Arbitrary inline title in single-quote form.
fn arb_sq_title() -> impl Strategy<Value = String> {
    "[A-Za-z0-9 ]{0,30}".prop_map(|s| format!("'{s}'"))
}

/// Arbitrary inline title in parenthesis form.
fn arb_paren_title() -> impl Strategy<Value = String> {
    "[A-Za-z0-9 ]{0,30}".prop_map(|s| format!("({s})"))
}

/// Arbitrary title in any supported delimiter form.
fn arb_title_form() -> impl Strategy<Value = String> {
    prop_oneof![arb_dq_title(), arb_sq_title(), arb_paren_title()]
}

/// Arbitrary title body containing an escaped quote or backslash.
fn arb_escaped_title_body() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("\\\"".to_string()),
        Just("\\\\".to_string()),
        "[A-Za-z0-9 ]{0,10}".prop_map(|s| format!("{s}\\\"tail")),
    ]
}

/// Arbitrary escaped double-quoted title.
fn arb_escaped_dq_title() -> impl Strategy<Value = String> {
    arb_escaped_title_body().prop_map(|body| format!("\"{body}\""))
}

proptest! {
    /// Any `[label]: URL` line (no title) must match and require a standalone title.
    #[test]
    fn valid_bare_link_ref_matches(label in arb_label(), url in arb_bare_url()) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("[{label}]: {url}");
        prop_assert!(matcher.is_definition(&line), "bare link ref should match: {line}");
        prop_assert!(
            matcher.standalone_title_need(&line) == Some(true),
            "bare link ref should need title: {line}"
        );
    }

    /// Angle-bracketed destinations must match bare link reference definitions.
    #[test]
    fn angle_bracket_url_matches(label in arb_label(), url in arb_angle_url()) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("[{label}]: {url}");
        prop_assert!(matcher.is_definition(&line), "angle URL link ref should match: {line}");
        prop_assert_eq!(matcher.standalone_title_need(&line), Some(true));
    }

    /// Indented definitions (0–3 columns) classify; four or more must not.
    #[test]
    fn definition_indentation_bounds(
        label in arb_label(),
        url in arb_bare_url(),
        indent in 0usize..=6usize,
    ) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("{:indent$}[{label}]: {url}", "", indent = indent);
        let expected = if indent < 4 {
            Some(BlockKind::LinkReferenceDefinition)
        } else {
            None
        };
        prop_assert!(
            classify_block(&line, matcher) == expected,
            "indent {indent} mismatch for: {line}"
        );
    }

    /// Any `[label]: URL "title"` line must match and NOT require a standalone title.
    #[test]
    fn valid_inline_title_link_ref_matches(
        label in arb_label(),
        url in arb_bare_url(),
        title in arb_dq_title(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("[{label}]: {url} {title}");
        prop_assert!(matcher.is_definition(&line), "inline-title link ref should match: {line}");
        prop_assert!(
            matcher.standalone_title_need(&line) == Some(false),
            "inline-title link ref should not need title: {line}"
        );
    }

    /// Single-quoted inline titles must suppress standalone title continuation.
    #[test]
    fn single_quoted_inline_title_matches(
        label in arb_label(),
        url in arb_bare_url(),
        title in arb_sq_title(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("[{label}]: {url} {title}");
        prop_assert!(matcher.is_definition(&line), "sq-title link ref should match: {line}");
        prop_assert_eq!(matcher.standalone_title_need(&line), Some(false));
    }

    /// Parenthesis inline titles must suppress standalone title continuation.
    #[test]
    fn parenthesis_inline_title_matches(
        label in arb_label(),
        url in arb_bare_url(),
        title in arb_paren_title(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("[{label}]: {url} {title}");
        prop_assert!(matcher.is_definition(&line), "paren-title link ref should match: {line}");
        prop_assert_eq!(matcher.standalone_title_need(&line), Some(false));
    }

    /// Escaped characters inside double-quoted titles must still match.
    #[test]
    fn escaped_inline_title_matches(
        label in arb_label(),
        url in arb_bare_url(),
        title in arb_escaped_dq_title(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("[{label}]: {url} {title}");
        prop_assert!(matcher.is_definition(&line), "escaped-title link ref should match: {line}");
        prop_assert_eq!(matcher.standalone_title_need(&line), Some(false));
    }

    /// Lines without a `[label]:` prefix must NOT match.
    #[test]
    fn plain_prose_does_not_match(text in "[A-Za-z ]{1,60}") {
        let matcher = LinkReferenceMatcher::production();
        // Ensure the text cannot accidentally start with `[`.
        prop_assume!(!text.trim_start().starts_with('['));
        prop_assert!(!matcher.is_definition(&text), "plain prose should not match: {text}");
    }

    /// Valid standalone title lines (any delimiter, ≤ 3 leading spaces) must match.
    #[test]
    fn valid_standalone_title_matches(
        spaces in 0usize..=3usize,
        title in arb_title_form(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("{:spaces$}{title}", "", spaces = spaces);
        prop_assert!(matcher.is_standalone_title_line(&line), "standalone title should match: {line}");
    }

    /// Escaped standalone titles must match when indentation is valid.
    #[test]
    fn escaped_standalone_title_matches(
        spaces in 0usize..=3usize,
        title in arb_escaped_dq_title(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("{:spaces$}{title}", "", spaces = spaces);
        prop_assert!(
            matcher.is_standalone_title_line(&line),
            "escaped standalone title should match: {line}"
        );
    }

    /// Four or more leading spaces must disqualify a standalone title.
    #[test]
    fn over_indented_standalone_title_does_not_match(
        extra in 4usize..=8usize,
        title in arb_title_form(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("{:spaces$}{title}", "", spaces = extra);
        prop_assert!(
            !matcher.is_standalone_title_line(&line),
            "over-indented title should not match: {line}"
        );
    }

    /// A closed window ignores subsequent lines until opened again.
    #[test]
    fn closed_window_is_inert(line in "\\PC*") {
        let matcher = LinkReferenceMatcher::production();
        let mut window = LinkTitleWindow::Closed;
        prop_assert_eq!(window.observe_next_line(&line, matcher), None);
        prop_assert_eq!(window, LinkTitleWindow::Closed);
    }

    /// Fence context always returns the window to `Closed`.
    #[test]
    fn fence_context_resets_window(
        prior in prop_oneof![
            Just(LinkTitleWindow::Closed),
            Just(LinkTitleWindow::AwaitingStandaloneTitle),
        ],
    ) {
        let mut window = prior;
        window.observe_fence_context();
        prop_assert_eq!(window, LinkTitleWindow::Closed);
    }

    /// Standalone title lines emit verbatim and close the window.
    #[test]
    fn awaiting_title_line_emits_verbatim(
        spaces in 0usize..=3usize,
        title in arb_title_form(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("{:spaces$}{title}", "", spaces = spaces);
        prop_assume!(matcher.is_standalone_title_line(&line));
        let mut window = LinkTitleWindow::AwaitingStandaloneTitle;
        prop_assert_eq!(
            window.observe_next_line(&line, matcher),
            Some(LinkTitleWindowOutcome::EmitVerbatim)
        );
        prop_assert_eq!(window, LinkTitleWindow::Closed);
    }

    /// Blank continuation lines emit verbatim and close the window.
    #[test]
    fn awaiting_blank_line_emits_verbatim(pad in 0usize..=4usize) {
        let matcher = LinkReferenceMatcher::production();
        let line = " ".repeat(pad);
        prop_assume!(line.trim().is_empty());
        let mut window = LinkTitleWindow::AwaitingStandaloneTitle;
        prop_assert_eq!(
            window.observe_next_line(&line, matcher),
            Some(LinkTitleWindowOutcome::EmitVerbatim)
        );
        prop_assert_eq!(window, LinkTitleWindow::Closed);
    }

    /// Non-title prose reprocesses through normal wrapping and closes the window.
    #[test]
    fn awaiting_prose_reprocesses(text in "[A-Za-z][A-Za-z0-9 ]{1,60}") {
        let matcher = LinkReferenceMatcher::production();
        prop_assume!(!matcher.is_standalone_title_line(&text));
        let mut window = LinkTitleWindow::AwaitingStandaloneTitle;
        prop_assert_eq!(
            window.observe_next_line(&text, matcher),
            Some(LinkTitleWindowOutcome::Reprocess)
        );
        prop_assert_eq!(window, LinkTitleWindow::Closed);
    }
}

#[test]
fn bare_definition_opens_window() {
    let mut window = LinkTitleWindow::Closed;
    window.observe_bare_definition();
    assert_eq!(window, LinkTitleWindow::AwaitingStandaloneTitle);
}
