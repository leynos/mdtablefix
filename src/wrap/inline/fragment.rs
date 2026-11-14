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
use unicode_width::UnicodeWidthStr;

use super::{
    ends_with_footnote_ref,
    fragment_is_link,
    is_inline_code_token,
    is_opening_punct,
    is_trailing_punct,
    is_whitespace_token,
    looks_like_bracketed_reference,
    looks_like_footnote_ref,
};
use crate::wrap::observer::{Event, FragmentKind, ObserverHandle};

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
    #[cfg(test)]
    pub(super) fn new(text: String) -> Self {
        let mut observer = crate::wrap::observer::NoOpObserver;
        Self::new_observed(text, &mut Some(&mut observer))
    }

    /// Builds a fragment from rendered `text`, reporting the classification.
    ///
    /// Identical to [`InlineFragment::new`] except that the computed
    /// `FragmentKind` is reported to `observer` as a `FragmentClassified`
    /// event; with a `None` handle nothing is emitted.
    pub(super) fn new_observed(text: String, observer: &mut ObserverHandle<'_>) -> Self {
        let width = UnicodeWidthStr::width(text.as_str());
        let kind = classify_fragment(text.as_str());
        if let Some(observer) = observer.as_deref_mut() {
            observer.observe(Event::FragmentClassified {
                token: text.as_str(),
                kind,
            });
        }
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
            FragmentKind::InlineCode
                | FragmentKind::Link
                | FragmentKind::FootnoteRef
                | FragmentKind::BracketedRef
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
///
/// The footnote probes below deliberately pass `&mut None` rather than the
/// caller's observer. They are speculative shape tests over three variants of
/// the same fragment, so forwarding the observer would emit up to three
/// `FootnoteRefChecked` events per fragment describing internal branch attempts
/// rather than an outcome. `FragmentClassified`, emitted once by
/// [`InlineFragment::new_observed`], is the single event reporting the result.
fn classify_fragment(text: &str) -> FragmentKind {
    if is_whitespace_token(text) {
        return FragmentKind::Whitespace;
    }
    let spellings = FragmentSpellings::of(text);
    if contains_link_with_trailing_punctuation(text) {
        FragmentKind::Link
    } else if spellings.is_any_code_shape() {
        FragmentKind::InlineCode
    } else if spellings.is_any_footnote_shape() {
        FragmentKind::FootnoteRef
    } else if spellings.is_any_bracketed_reference() {
        FragmentKind::BracketedRef
    } else {
        FragmentKind::Plain
    }
}

/// The three spellings of one rendered fragment that classification probes.
///
/// A wrapped token keeps its trailing sentence punctuation, its opening
/// bracket or quote, and sometimes an inflectional suffix after a closing
/// fence. Every probe below therefore tests the same three strings, and each
/// probe must test the same ones: deriving them once, here, is what keeps the
/// code, footnote, and bracketed-reference probes from drifting apart.
struct FragmentSpellings<'a> {
    /// The fragment exactly as rendered.
    text: &'a str,
    /// `text` with trailing punctuation removed.
    trimmed: &'a str,
    /// `trimmed` with leading opening punctuation removed.
    without_opening: &'a str,
}

impl<'a> FragmentSpellings<'a> {
    /// Derives the three spellings from the rendered fragment.
    fn of(text: &'a str) -> Self {
        let trimmed = text.trim_end_matches(is_trailing_punct);
        Self {
            text,
            trimmed,
            without_opening: trimmed.trim_start_matches(is_opening_punct),
        }
    }

    /// Reports whether any spelling is an inline code span.
    ///
    /// A span may carry trailing punctuation, opening punctuation, or an
    /// inflectional suffix after its closing fence, so each spelling is tested
    /// rather than only the original text.
    fn is_any_code_shape(&self) -> bool {
        is_inline_code_token(self.text)
            || is_inline_code_token(self.trimmed)
            || is_inline_code_token(self.without_opening)
            || has_inline_code_structure(self.text)
    }

    /// Reports whether the fragment is a footnote reference in any spelling.
    ///
    /// The `word.[^label]` suffix shape is included because the wrapper keeps
    /// that punctuation attached to the marker.
    fn is_any_footnote_shape(&self) -> bool {
        looks_like_footnote_ref(self.text, &mut None)
            || looks_like_footnote_ref(self.trimmed, &mut None)
            || ends_with_footnote_ref(self.text, &mut None)
    }

    /// Reports whether the fragment is a bare bracketed reference in any spelling.
    fn is_any_bracketed_reference(&self) -> bool {
        looks_like_bracketed_reference(self.text) || looks_like_bracketed_reference(self.trimmed)
    }
}
#[cfg(test)]
#[path = "fragment_tests.rs"]
mod tests;

#[cfg(test)]
mod tracing_tests {
    //! Traced-event tests for `InlineFragment` classification.
    //!
    //! Each test verifies that constructing an `InlineFragment` emits a DEBUG
    //! `fragment classified` event with the correct content-free fields
    //! (`kind`, `token_length`) and that the fragment text never reaches the
    //! log. One test verifies that construction succeeds without any tracing
    //! subscriber installed.

    use rstest::rstest;
    // Wrapper over `tracing_test::traced_test`; see `test_macros` for why.
    use test_macros::traced_test;

    use super::InlineFragment;
    use crate::wrap::{observer::FragmentKind, tracing_adapter::TracingObserver};

    #[traced_test]
    #[rstest]
    #[case("[^1]", "FootnoteRef")]
    #[case("`code`", "InlineCode")]
    #[case("[text](https://example.com)", "Link")]
    #[case("   ", "Whitespace")]
    #[case("plain", "Plain")]
    fn fragment_classification_logs_kind(#[case] input: &str, #[case] expected: &str) {
        let mut observer = TracingObserver;
        let _fragment = InlineFragment::new_observed(input.to_string(), &mut Some(&mut observer));
        assert!(logs_contain("fragment classified"));
        assert!(logs_contain(&format!("kind={expected}")));
        assert!(logs_contain(&format!(
            "token_length={}",
            input.chars().count()
        )));
    }

    #[traced_test]
    #[test]
    fn fragment_classification_does_not_log_fragment_text() {
        let mut observer = TracingObserver;
        let text = "[unmistakable-label](https://example.com/unmistakable-path)";
        let _fragment = InlineFragment::new_observed(text.to_string(), &mut Some(&mut observer));
        assert!(logs_contain("fragment classified"));
        assert!(!logs_contain("unmistakable-label"));
        assert!(!logs_contain("unmistakable-path"));
    }

    // Drives `TracingObserver` with no subscriber active — the configuration
    // production callers use. Going through `InlineFragment::new` instead would
    // attach `NoOpObserver` and exercise a different path entirely, never
    // reaching the adapter.
    //
    // The no-subscriber state is established explicitly rather than by omitting
    // `#[traced_test]`. `tracing_test` installs a *global* dispatcher behind a
    // `Once`, and this module's own traced tests share this binary, so by the
    // time this test runs one is likely already installed. A thread-local
    // `NoSubscriber` takes precedence over that global.
    #[test]
    fn fragment_classification_with_observer_but_no_subscriber_returns_kind() {
        let fragment =
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                let mut observer = TracingObserver;
                let mut handle = Some(&mut observer as &mut dyn crate::wrap::observer::Observer);
                InlineFragment::new_observed("[^1]".to_string(), &mut handle)
            });
        assert_eq!(fragment.kind, FragmentKind::FootnoteRef);
    }
}
#[cfg(test)]
#[path = "fragment_tracing_snapshots.rs"]
mod fragment_tracing_snapshots;
