//! Inline fragment types used by Markdown-aware wrapping.
//!
//! This module defines the small units passed from `wrap_preserving_code` to
//! `textwrap`. Each `InlineFragment` stores rendered text, display width, and
//! a `FragmentKind` classification so the wrapper can distinguish whitespace,
//! ordinary prose, and Markdown syntax that must remain atomic.
//!
//! The classification here feeds `inline::postprocess`, which uses cheap
//! predicates such as `is_atomic` and `is_plain` to merge whitespace artefacts
//! and rebalance tails after greedy line fitting. Keeping the fragment model in
//! one module avoids repeating link, code, and GFM footnote-reference detection
//! throughout the wrapping pipeline.

use textwrap::core::Fragment;
use tracing::debug;
use unicode_width::UnicodeWidthStr;

use super::{
    ends_with_footnote_ref,
    fragment_is_link,
    is_inline_code_token,
    is_opening_punct,
    is_trailing_punct,
    is_whitespace_token,
    looks_like_footnote_ref,
};

/// Classifies an inline fragment for post-wrap heuristics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FragmentKind {
    /// Marks a fragment that contains only whitespace.
    Whitespace,
    /// Marks a fragment that contains inline code.
    InlineCode,
    /// Marks a fragment that contains a Markdown link.
    Link,
    /// Marks a fragment that contains a GFM footnote reference.
    FootnoteRef,
    /// Marks a fragment that contains ordinary prose.
    Plain,
}

/// Stores rendered fragment text, width, and classification for wrapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InlineFragment {
    /// Holds the rendered fragment text that will be emitted unchanged.
    pub(super) text: String,
    /// Stores the precomputed Unicode display width for `text`.
    pub(super) width: usize,
    /// Records the fragment classification used by post-processing predicates.
    pub(super) kind: FragmentKind,
}

impl InlineFragment {
    /// Builds a fragment from rendered `text`.
    ///
    /// The parameter is stored verbatim. The returned fragment also carries
    /// its Unicode display width, computed with `UnicodeWidthStr::width`, and
    /// its `FragmentKind`, computed once through `classify_fragment`.
    pub(super) fn new(text: String) -> Self {
        let width = UnicodeWidthStr::width(text.as_str());
        let kind = classify_fragment(text.as_str());
        log_fragment_classification(text.as_str(), &kind);
        Self { text, width, kind }
    }

    /// Returns whether this fragment contains only whitespace.
    ///
    /// This `pub(super)` predicate is true only when `self.kind` is
    /// `FragmentKind::Whitespace`, meaning the fragment represents whitespace
    /// characters and can be merged by whitespace post-processing.
    pub(super) fn is_whitespace(&self) -> bool { self.kind == FragmentKind::Whitespace }

    /// Returns whether this fragment must move as an atomic unit.
    pub(super) fn is_atomic(&self) -> bool {
        matches!(
            self.kind,
            FragmentKind::InlineCode | FragmentKind::Link | FragmentKind::FootnoteRef
        )
    }

    /// Returns whether this fragment is ordinary prose.
    pub(super) fn is_plain(&self) -> bool { self.kind == FragmentKind::Plain }
}

impl Fragment for InlineFragment {
    /// Returns this fragment's display width as required by `textwrap`.
    ///
    /// Widths larger than `u32::MAX` are clamped by `width_as_f64` before
    /// conversion. Real fragments are measured from strings, so reaching that
    /// cap would require pathological input; the clamp keeps conversion
    /// infallible for the trait method.
    fn width(&self) -> f64 { width_as_f64(self.width) }
    fn whitespace_width(&self) -> f64 { 0.0 }
    fn penalty_width(&self) -> f64 { 0.0 }
}

/// Converts a display width into the `f64` representation required by
/// `textwrap`.
///
/// Values larger than `u32::MAX` are silently clamped before conversion. That
/// defensive fallback matters on 64-bit platforms: a pathological `usize`
/// width becomes roughly 4.29e9 columns instead of panicking, so `textwrap`
/// receives a capped width and may produce unexpected layout for callers that
/// rely on extremely large widths.
pub(super) fn width_as_f64(width: usize) -> f64 {
    f64::from(u32::try_from(width).unwrap_or(u32::MAX))
}

/// Returns whether `text` begins with a matched inline code fence, optionally
/// followed by a non-whitespace suffix such as an inflectional affix.
pub(super) fn has_inline_code_structure(text: &str) -> bool {
    fn matched_fence(text: &str) -> bool {
        let fence_len = text.chars().take_while(|&ch| ch == '`').count();
        if fence_len == 0 {
            return false;
        }
        // SAFETY: backtick (U+0060) is a one-byte ASCII codepoint, so the
        // character count from `take_while` equals the byte length of the
        // fence prefix. Slicing by `fence_len` is therefore a valid UTF-8
        // boundary and will not panic.
        let fence = &text[..fence_len];
        text[fence_len..].contains(fence)
    }

    let trimmed = text.trim_end_matches(is_trailing_punct);
    let without_opening = trimmed.trim_start_matches(is_opening_punct);
    matched_fence(text) || matched_fence(trimmed) || matched_fence(without_opening)
}

/// Returns whether `text` is a link plus optional trailing punctuation.
fn contains_link_with_trailing_punctuation(text: &str) -> bool {
    let mut candidate = text;

    loop {
        if fragment_is_link(candidate) {
            return true;
        }

        let Some(ch) = candidate.chars().next_back() else {
            return false;
        };
        if !is_trailing_punct(ch) {
            return false;
        }
        candidate = &candidate[..candidate.len() - ch.len_utf8()];
    }
}

/// Classifies rendered fragment `text` for later post-processing.
///
/// `classify_fragment` checks both the original text and a copy trimmed of
/// trailing punctuation, so tokens such as `[link](url).` and `` `code`, `` are
/// still recognised as links or code spans. Footnote references also recognise
/// the `word.[^label]` suffix shape that the wrapper groups to avoid splitting
/// sentence punctuation from the marker.
fn classify_fragment(text: &str) -> FragmentKind {
    if is_whitespace_token(text) {
        return FragmentKind::Whitespace;
    }
    let trimmed = text.trim_end_matches(is_trailing_punct);
    let without_opening = trimmed.trim_start_matches(is_opening_punct);
    if contains_link_with_trailing_punctuation(text) {
        FragmentKind::Link
    } else if is_inline_code_token(text)
        || is_inline_code_token(trimmed)
        || is_inline_code_token(without_opening)
        || has_inline_code_structure(text)
    {
        FragmentKind::InlineCode
    } else if looks_like_footnote_ref(text)
        || looks_like_footnote_ref(trimmed)
        || ends_with_footnote_ref(text)
    {
        FragmentKind::FootnoteRef
    } else {
        FragmentKind::Plain
    }
}

/// Returns a UTF-8-safe prefix of `text` for debug logging.
///
/// The prefix contains at most 80 bytes and never splits a multi-byte
/// character. The second tuple element is `true` when `text` was shortened.
fn trace_text_snippet(text: &str) -> (&str, bool) {
    const MAX_TRACE_BYTES: usize = 80;
    if text.len() <= MAX_TRACE_BYTES {
        return (text, false);
    }

    let mut byte_end = 0;
    for (idx, ch) in text.char_indices() {
        let next_end = idx + ch.len_utf8();
        if next_end > MAX_TRACE_BYTES {
            break;
        }
        byte_end = next_end;
    }

    (&text[..byte_end], true)
}

/// Emits a structured trace when fragment classification logging is enabled.
fn log_fragment_classification(text: &str, kind: &FragmentKind) {
    if tracing::enabled!(tracing::Level::DEBUG) {
        let (snippet, truncated) = trace_text_snippet(text);
        debug!(
            token = %snippet,
            truncated,
            kind = ?kind,
            "fragment classified"
        );
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// has_inline_code_structure must not panic on arbitrary Unicode input.
        #[test]
        fn has_inline_code_structure_never_panics(text in "\\PC*") {
            let _ = has_inline_code_structure(&text);
        }

        /// A string that starts and ends with a matching backtick fence and
        /// contains no embedded fence of the same length must satisfy the predicate.
        #[test]
        fn has_inline_code_structure_detects_simple_fence(
            inner in "[^`]{1,40}",
        ) {
            let text = format!("`{inner}`");
            prop_assert!(has_inline_code_structure(&text));
        }
    }

    // Fence preceded by an opening bracket is still detected (without_opening path).
    proptest! {
        #[test]
        fn has_inline_code_structure_detects_opening_punct_prefix(
            inner in "[^`]{1,40}",
        ) {
            // Opening punctuation trimmed before fence detection.
            let text = format!("(`{inner}`)");
            prop_assert!(has_inline_code_structure(&text));
        }
    }

    // Fence followed by trailing punctuation is still detected (trimmed path).
    proptest! {
        #[test]
        fn has_inline_code_structure_detects_trailing_punct_suffix(
            inner in "[^`]{1,40}",
        ) {
            let text = format!("`{inner}`.");
            prop_assert!(has_inline_code_structure(&text));
        }
    }

    // Fences longer than one backtick are detected.
    proptest! {
        #[test]
        fn has_inline_code_structure_detects_multi_char_fence(
            inner in "[^`]{1,20}",
            fence_len in 2usize..=4usize,
        ) {
            let fence: String = "`".repeat(fence_len);
            let text = format!("{fence}{inner}{fence}");
            prop_assert!(has_inline_code_structure(&text));
        }
    }

    // Possessive suffix does not prevent detection.
    proptest! {
        #[test]
        fn has_inline_code_structure_with_possessive_suffix(
            inner in "[^`]{1,20}",
        ) {
            let text = format!("`{inner}`'s");
            prop_assert!(has_inline_code_structure(&text));
        }
    }

    // Hyphenated compound suffix does not prevent detection.
    proptest! {
        #[test]
        fn has_inline_code_structure_with_hyphen_suffix(
            inner in "[^`]{1,20}",
            word  in "[a-z]{2,10}",
        ) {
            let text = format!("`{inner}`-{word}");
            prop_assert!(has_inline_code_structure(&text));
        }
    }

    // A Markdown link must NOT be classified as inline code structure.
    // This guards against false positives when link text contains backticks.
    proptest! {
        #[test]
        fn has_inline_code_structure_does_not_match_plain_link(
            label in "[a-zA-Z]{1,20}",
            url   in "https://[a-z]{3,10}\\.[a-z]{2,4}",
        ) {
            // A plain link with no backticks in the label must not match.
            let text = format!("[{label}]({url})");
            assert!(!has_inline_code_structure(&text));
        }
    }
}

#[cfg(test)]
mod trace_snippet_tests {
    //! Tests for the `trace_text_snippet` helper.
    //!
    //! Verifies that the UTF-8-safe truncation helper produces a slice at a
    //! valid character boundary and sets the truncation flag correctly.

    use super::trace_text_snippet;

    #[test]
    fn trace_text_snippet_truncates_on_char_boundary() {
        let ascii = "a".repeat(79);
        let text = format!("{ascii}étail");
        let (snippet, truncated) = trace_text_snippet(&text);

        assert!(truncated);
        assert_eq!(snippet, ascii.as_str());
        assert!(snippet.is_char_boundary(snippet.len()));
    }
}

#[cfg(test)]
mod tracing_tests {
    //! Traced-event tests for `InlineFragment` classification.
    //!
    //! Each test verifies that constructing an `InlineFragment` emits a DEBUG
    //! `fragment classified` event with the correct structured fields (`kind`,
    //! `token`, `truncated`).  One test verifies that construction succeeds
    //! without any tracing subscriber installed.

    use rstest::rstest;
    use tracing_test::traced_test;

    use super::{FragmentKind, InlineFragment};

    #[traced_test]
    #[rstest]
    #[case("[^1]", "FootnoteRef")]
    #[case("`code`", "InlineCode")]
    #[case("[text](https://example.com)", "Link")]
    #[case("   ", "Whitespace")]
    #[case("plain", "Plain")]
    fn fragment_classification_logs_kind(#[case] input: &str, #[case] expected: &str) {
        let _fragment = InlineFragment::new(input.to_string());
        assert!(logs_contain("fragment classified"));
        assert!(logs_contain(&format!("kind={expected}")));
        assert!(logs_contain("token="));
        assert!(logs_contain("truncated="));
    }

    #[test]
    fn fragment_classification_does_not_require_subscriber() {
        let fragment = InlineFragment::new("[^1]".to_string());
        assert_eq!(fragment.kind, FragmentKind::FootnoteRef);
    }
}

#[cfg(test)]
mod proptests {
    //! Property tests for `trace_text_snippet` invariants.
    //!
    //! Verifies on arbitrary Unicode input that the helper never panics, the
    //! result is a valid UTF-8 slice of at most 80 bytes, and the truncation
    //! flag accurately reflects whether the input exceeded that limit.

    use proptest::prelude::*;

    use super::trace_text_snippet;

    proptest! {
        #[test]
        fn trace_text_snippet_never_panics(s in "\\PC*") {
            let (snippet, truncated) = trace_text_snippet(&s);
            // Invariant 1: result is always valid UTF-8 at a char boundary.
            assert!(snippet.is_char_boundary(snippet.len()));
            // Invariant 2: result never exceeds 80 bytes.
            assert!(snippet.len() <= 80);
            // Invariant 3: truncation flag is accurate.
            assert_eq!(truncated, s.len() > 80);
        }
    }
}
