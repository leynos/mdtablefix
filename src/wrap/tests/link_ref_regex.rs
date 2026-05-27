//! Property-based tests for link reference regex correctness.

use proptest::prelude::*;

use crate::wrap::link_reference::LinkReferenceMatcher;

/// Arbitrary label: one or more non-`]` printable ASCII characters.
fn arb_label() -> impl Strategy<Value = String> { "[A-Za-z0-9 _-]{1,40}".prop_map(|s| s) }

/// Arbitrary bare URL: starts with `https://` followed by alphanumerics.
fn arb_bare_url() -> impl Strategy<Value = String> {
    "[a-z]{3,8}://[A-Za-z0-9./-]{1,40}".prop_map(|s| s)
}

/// Arbitrary inline title in double-quote form.
fn arb_dq_title() -> impl Strategy<Value = String> {
    "[A-Za-z0-9 ]{0,30}".prop_map(|s| format!("\"{s}\""))
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

    /// Lines without a `[label]:` prefix must NOT match.
    #[test]
    fn plain_prose_does_not_match(text in "[A-Za-z ]{1,60}") {
        let matcher = LinkReferenceMatcher::production();
        // Ensure the text cannot accidentally start with `[`.
        prop_assume!(!text.trim_start().starts_with('['));
        prop_assert!(!matcher.is_definition(&text), "plain prose should not match: {text}");
    }

    /// Valid standalone title lines (double-quote form, ≤ 3 leading spaces) must match.
    #[test]
    fn valid_standalone_title_matches(
        spaces in 0usize..=3usize,
        title in arb_dq_title(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("{:spaces$}{title}", "", spaces = spaces);
        prop_assert!(matcher.is_standalone_title_line(&line), "standalone title should match: {line}");
    }

    /// Four or more leading spaces must disqualify a standalone title.
    #[test]
    fn over_indented_standalone_title_does_not_match(
        extra in 4usize..=8usize,
        title in arb_dq_title(),
    ) {
        let matcher = LinkReferenceMatcher::production();
        let line = format!("{:spaces$}{title}", "", spaces = extra);
        prop_assert!(
            !matcher.is_standalone_title_line(&line),
            "over-indented title should not match: {line}"
        );
    }
}
