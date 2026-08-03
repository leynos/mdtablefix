//! Property tests for the inline observer boundary.
//!
//! Fixed traced tests cover individual events; these properties cover the
//! boundary's structural guarantees over generated inline Markdown:
//!
//! - attaching an observer never changes wrapped output,
//! - observation is deterministic for a given input,
//! - reported metadata is internally consistent, and
//! - constructs that must be reported are reported.

use proptest::prelude::*;

use crate::wrap::{
    inline::wrapping::wrap_preserving_code_observed,
    observer::{Event, Observer},
};

/// One recorded event, reduced to owned, content-free metadata.
///
/// Only the event name and its numeric fields are kept. Token text is
/// deliberately not stored, mirroring the rule that observers record metadata
/// rather than document content.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EventSummary {
    name: &'static str,
    token_length: Option<usize>,
}

/// Collects events in memory so tests can assert on what the domain reported.
#[derive(Debug, Default)]
struct RecordingObserver {
    events: Vec<EventSummary>,
}

impl RecordingObserver {
    fn names(&self) -> Vec<&'static str> { self.events.iter().map(|e| e.name).collect() }
}

impl Observer for RecordingObserver {
    fn observe(&mut self, event: Event<'_>) {
        let (name, token_length) = match event {
            Event::FootnoteReferenceParsed { token } => {
                ("FootnoteReferenceParsed", Some(token.chars().count()))
            }
            Event::LinkOrImageParsed { token, .. } => {
                ("LinkOrImageParsed", Some(token.chars().count()))
            }
            Event::FootnoteEndNotFound { .. } => ("FootnoteEndNotFound", None),
            Event::FootnoteLabelRecognized { token, .. } => {
                ("FootnoteLabelRecognized", Some(token.chars().count()))
            }
            Event::FootnoteRefChecked { token, .. } => {
                ("FootnoteRefChecked", Some(token.chars().count()))
            }
            Event::DateSequenceMatched { .. } => ("DateSequenceMatched", None),
            Event::DateSequenceGrouped { .. } => ("DateSequenceGrouped", None),
            Event::WhitespaceFootnoteCoupling { token, .. } => {
                ("WhitespaceFootnoteCoupling", Some(token.chars().count()))
            }
            Event::FootnoteReferenceCoupling { token, .. } => {
                ("FootnoteReferenceCoupling", Some(token.chars().count()))
            }
            Event::FragmentClassified { token, .. } => {
                ("FragmentClassified", Some(token.chars().count()))
            }
        };
        self.events.push(EventSummary { name, token_length });
    }
}

/// Wraps `text` with no observer attached.
fn wrap_unobserved(text: &str, width: usize) -> Vec<String> {
    wrap_preserving_code_observed(text, width, &mut None)
}

/// Wraps `text` while recording every reported event.
fn wrap_recorded(text: &str, width: usize) -> (Vec<String>, RecordingObserver) {
    let mut recorder = RecordingObserver::default();
    let lines = wrap_preserving_code_observed(text, width, &mut Some(&mut recorder));
    (lines, recorder)
}

fn word() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,8}").expect("word strategy")
}

fn inline_piece() -> impl Strategy<Value = String> {
    prop_oneof![
        word(),
        word().prop_map(|w| format!("`{w}`")),
        (word(), word()).prop_map(|(label, path)| format!("[{label}](https://example.com/{path})")),
        word().prop_map(|label| format!("[^{label}]")),
        word().prop_map(|w| format!("{w}.")),
    ]
}

fn inline_markdown() -> impl Strategy<Value = String> {
    prop::collection::vec(inline_piece(), 1..12).prop_map(|pieces| pieces.join(" "))
}

proptest! {
    /// Attaching an observer must not change wrapping output: the boundary is a
    /// diagnostics channel, never a participant in layout.
    #[test]
    fn observation_does_not_change_output(
        text in inline_markdown(),
        width in 8usize..=100,
    ) {
        let unobserved = wrap_unobserved(&text, width);
        let (observed, _) = wrap_recorded(&text, width);
        prop_assert_eq!(unobserved, observed);
    }

    /// Observing the same input twice reports the same events in the same
    /// order, so snapshots and log-based assertions are stable.
    #[test]
    fn observation_is_deterministic(
        text in inline_markdown(),
        width in 8usize..=100,
    ) {
        let (first_lines, first) = wrap_recorded(&text, width);
        let (second_lines, second) = wrap_recorded(&text, width);
        prop_assert_eq!(first_lines, second_lines);
        prop_assert_eq!(first.events, second.events);
    }

    /// Every event that carries a token reports a non-zero character count, so
    /// no event describes an empty token.
    #[test]
    fn reported_token_lengths_are_non_zero(
        text in inline_markdown(),
        width in 8usize..=100,
    ) {
        let (_, recorder) = wrap_recorded(&text, width);
        for event in &recorder.events {
            if let Some(length) = event.token_length {
                prop_assert!(
                    length > 0,
                    "event {} reported an empty token", event.name
                );
            }
        }
    }

    /// Any non-empty input yields at least one classified fragment, so the
    /// boundary never falls silent on real work.
    #[test]
    fn wrapping_always_reports_fragment_classification(
        text in inline_markdown(),
        width in 8usize..=100,
    ) {
        let (lines, recorder) = wrap_recorded(&text, width);
        prop_assert!(!lines.is_empty());
        prop_assert!(
            recorder.names().contains(&"FragmentClassified"),
            "expected a FragmentClassified event, got {:?}",
            recorder.names()
        );
    }

    /// A generated footnote reference is always reported as parsed, proving
    /// events propagate from the tokenizer through the wrapping pipeline.
    #[test]
    fn footnote_references_are_reported(
        label in "[a-z]{1,8}",
        width in 8usize..=100,
    ) {
        let text = format!("alpha [^{label}] omega");
        let (_, recorder) = wrap_recorded(&text, width);
        prop_assert!(
            recorder.names().contains(&"FootnoteReferenceParsed"),
            "expected a FootnoteReferenceParsed event, got {:?}",
            recorder.names()
        );
    }
}
