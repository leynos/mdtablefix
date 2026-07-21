//! Unit and property tests for higher-level inline Markdown parsing helpers.

use proptest::prelude::*;
use rstest::rstest;

use super::*;

fn footnote_label_part_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            (b'a'..=b'z').prop_map(char::from),
            (b'A'..=b'Z').prop_map(char::from),
            (b'0'..=b'9').prop_map(char::from),
            Just('-'),
            Just('_')
        ],
        0..12,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

/// A fragment of link-text content that never contains an *unescaped* `]`.
///
/// Each fragment is either a plain, safe character (letters, brackets, spaces,
/// or parentheses — but never a bare backslash or a bare `]`) or a backslash
/// escape `\x`. A literal `]` is emitted only as the escaped pair `\]`, and the
/// only fragment ending in backslashes is `\\` (an even run). Any `]` in the
/// concatenation is therefore preceded by an odd number of backslashes and is
/// escaped, so the concatenation contains no unescaped `]`. Unescaped `[`
/// characters may appear freely, exercising nested-bracket content.
fn link_text_safe_fragment() -> impl Strategy<Value = String> {
    prop_oneof![
        prop_oneof![
            (b'a'..=b'z').prop_map(char::from),
            Just('['),
            Just('('),
            Just(')'),
            Just(' '),
        ]
        .prop_map(|ch| ch.to_string()),
        prop_oneof![Just(']'), Just('['), Just('\\'), Just('('), Just('a')]
            .prop_map(|escaped| format!("\\{escaped}")),
    ]
}

fn link_text_safe_inner() -> impl Strategy<Value = String> {
    prop::collection::vec(link_text_safe_fragment(), 0..8).prop_map(|parts| parts.concat())
}

/// A fragment of link-URL content that keeps the parenthesis depth at or above
/// the entry depth and always returns to it.
///
/// Leaves are plain letters or escaped delimiters (`\(`, `\)`, `\\`), none of
/// which change depth; recursive nodes wrap a balanced sub-sequence in a literal
/// `(...)` pair. Placed between the outer parentheses (entry depth 1), such
/// content never brings the depth to 0 before the matching outer `)`.
fn url_fragment() -> impl Strategy<Value = String> {
    let leaf = prop_oneof![
        (b'a'..=b'z').prop_map(|byte| char::from(byte).to_string()),
        prop_oneof![Just('('), Just(')'), Just('\\'), Just('a')]
            .prop_map(|escaped| format!("\\{escaped}")),
    ];
    leaf.prop_recursive(4, 24, 4, |fragment| {
        prop::collection::vec(fragment, 0..4).prop_map(|parts| format!("({})", parts.concat()))
    })
}

fn balanced_url_inner() -> impl Strategy<Value = String> {
    prop::collection::vec(url_fragment(), 0..5).prop_map(|parts| parts.concat())
}

#[test]
fn parse_link_or_image_handles_nested_parentheses() {
    let text = "![alt](path(a(b)c)) more";
    let (token, idx) = parse_link_or_image(text, 0);
    assert_eq!(token, "![alt](path(a(b)c))");
    assert_eq!(idx, token.len());
}
#[test]
fn parse_link_or_image_falls_back_on_malformed_input() {
    let text = "[broken";
    let (token, idx) = parse_link_or_image(text, 0);
    assert_eq!(token, "[");
    assert_eq!(idx, "[".len());
}
#[test]
fn parse_link_or_image_handles_deeply_nested_parentheses() {
    let text = "[link](url(a(b(c)d)e)) tail";
    let (token, idx) = parse_link_or_image(text, 0);
    assert_eq!(token, "[link](url(a(b(c)d)e))");
    assert_eq!(idx, token.len());
}

#[test]
fn parse_link_or_image_handles_nested_parentheses_for_images() {
    let text = "![alt](path(a(b(c)d)e))";
    let (token, idx) = parse_link_or_image(text, 0);
    assert_eq!(token, "![alt](path(a(b(c)d)e))");
    assert_eq!(idx, token.len());
}

#[test]
fn parse_link_or_image_handles_text_ending_at_bracket() {
    let text = "[";
    let (token, idx) = parse_link_or_image(text, 0);
    assert_eq!(token, "[");
    assert_eq!(idx, 1);
}

#[test]
fn parse_link_or_image_preserves_footnote_reference() {
    let text = "[^4] tail";
    let (token, idx) = parse_link_or_image(text, 0);
    assert_eq!(token, "[^4]");
    assert_eq!(idx, token.len());
}

#[test]
fn parse_link_or_image_preserves_footnote_reference_with_escaped_bracket() {
    let text = r"[^a\]b] tail";
    let (token, idx) = parse_link_or_image(text, 0);
    assert_eq!(token, r"[^a\]b]");
    assert_eq!(idx, token.len());
}

#[test]
fn parse_link_or_image_preserves_footnote_at_end() {
    let text = "[^4]";
    let (token, idx) = parse_link_or_image(text, 0);
    assert_eq!(token, "[^4]");
    assert_eq!(idx, token.len());
}

#[test]
fn parse_link_or_image_keeps_caret_text_links_as_links() {
    let text = "[^label](https://example.com) tail";
    let (token, idx) = parse_link_or_image(text, 0);
    assert_eq!(token, "[^label](https://example.com)");
    assert_eq!(idx, token.len());
}

#[rstest]
fn parse_link_or_image_preserves_reference_style_link() {
    let input = "[trybuild][implicit-fixture-trybuild]";

    assert_eq!(
        parse_link_or_image(input, 0),
        (input.to_string(), input.len())
    );
}

proptest! {
    #[test]
    fn parse_link_or_image_preserves_footnote_references_with_escaped_brackets(
        prefix in footnote_label_part_strategy(),
        suffix in footnote_label_part_strategy(),
    ) {
        let expected = format!(r"[^{prefix}\]{suffix}]");
        let expected_len = expected.len();
        let text = format!("{expected} tail");

        let (token, idx) = parse_link_or_image(&text, 0);

        prop_assert_eq!(token, expected);
        prop_assert_eq!(idx, expected_len);
    }

    /// An unescaped `]` — and only an unescaped `]` — terminates a link-text
    /// span. Escaped `]` (behind arbitrary odd backslash runs) and unescaped
    /// nested `[` are ordinary content, so the parser stops at the first
    /// appended terminator regardless of any `]` in the trailing text.
    #[test]
    fn parse_link_text_terminates_at_first_unescaped_bracket(
        inner in link_text_safe_inner(),
        tail in r"[a-z\]]{0,5}",
    ) {
        let text = format!("[{inner}]{tail}");
        let expected_end = format!("[{inner}]").len();
        prop_assert_eq!(parse_link_text(&text, 0), Some(expected_end));
    }

    /// The parity of a backslash run immediately before `]` decides whether the
    /// bracket is escaped: an even run leaves it as a terminator, while an odd
    /// run escapes it so the span never closes.
    #[test]
    fn parse_link_text_respects_backslash_parity(run in 0usize..8) {
        let text = format!("[{}]", "\\".repeat(run));
        let result = parse_link_text(&text, 0);
        if run % 2 == 0 {
            prop_assert_eq!(result, Some(text.len()));
        } else {
            prop_assert_eq!(result, None);
        }
    }

    /// A balanced, arbitrarily nested destination closes at the outer `)` that
    /// returns the depth to zero. Escaped parentheses leave the depth unchanged,
    /// and trailing text after the close is never consumed.
    #[test]
    fn parse_link_url_closes_on_balanced_destination(
        inner in balanced_url_inner(),
        tail in "[a-z ]{0,5}",
    ) {
        let text = format!("({inner}){tail}");
        let expected_end = format!("({inner})").len();
        prop_assert_eq!(parse_link_url(&text, 0), Some(expected_end));
    }

    /// The parity of a backslash run immediately before `)` decides whether the
    /// parenthesis closes the destination: an even run closes depth 1, while an
    /// odd run escapes the `)` so the destination never closes.
    #[test]
    fn parse_link_url_respects_backslash_parity(run in 0usize..8) {
        let text = format!("({})", "\\".repeat(run));
        let result = parse_link_url(&text, 0);
        if run % 2 == 0 {
            prop_assert_eq!(result, Some(text.len()));
        } else {
            prop_assert_eq!(result, None);
        }
    }
}

mod tracing_tests {
    //! Traced-event tests for the parsing helpers.
    //!
    //! Verifies that `parse_link_or_image` and `find_footnote_end` emit
    //! the expected DEBUG and TRACE events, including structured fields,
    //! for all reachable branches: footnote reference parsed, link or
    //! image parsed, prefix mismatch, footnote label span recognized, and
    //! unterminated bracket.

    use tracing_test::traced_test;

    use super::*;

    #[traced_test]
    #[test]
    fn parse_link_or_image_logs_footnote_reference() {
        let _ = parse_link_or_image("[^4] tail", 0);
        assert!(logs_contain("footnote reference parsed"));
        assert!(logs_contain("token_length=4"));
        assert!(!logs_contain("[^4]"));
    }

    #[traced_test]
    #[test]
    fn parse_link_or_image_logs_link_parsed() {
        let _ = parse_link_or_image("[link](url)", 0);
        assert!(logs_contain("link or image parsed"));
        assert!(logs_contain("token_length=11"));
        assert!(!logs_contain("[link](url)"));
        assert!(logs_contain("is_image="));
    }

    #[traced_test]
    #[test]
    fn find_footnote_end_logs_prefix_mismatch() {
        let _ = find_footnote_end("no-caret", 0);
        assert!(logs_contain("footnote end not found"));
        assert!(logs_contain("reason="));
        assert!(logs_contain("prefix_mismatch"));
    }

    #[traced_test]
    #[test]
    fn parse_link_or_image_logs_footnote_label_span() {
        let _ = parse_link_or_image("[^4] tail", 0);
        assert!(logs_contain("footnote label span recognized"));
        assert!(logs_contain("start="));
        assert!(logs_contain("end="));
        assert!(logs_contain("token_length=4"));
        assert!(!logs_contain("[^4]"));
    }

    #[traced_test]
    #[test]
    fn find_footnote_end_logs_unterminated_bracket() {
        let _ = find_footnote_end("[^unterminated", 0);
        assert!(logs_contain("footnote end not found"));
        assert!(logs_contain("reason="));
        assert!(logs_contain("unterminated_bracket"));
    }
}
