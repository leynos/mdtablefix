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
    observer::{Event, FragmentKind, Observer, SpanKind},
};

/// One recorded event, reduced to owned, content-free metadata.
///
/// Every stable field of every [`Event`] variant is captured, except the token
/// text itself, which is reduced to a character count. Token text is
/// deliberately not stored, mirroring the rule that observers record metadata
/// rather than document content.
///
/// The conversion below destructures each variant exhaustively, without a `..`
/// rest pattern. Adding a field to an `Event` therefore fails to compile until
/// it is summarized here, so no field can silently escape
/// [`observation_is_deterministic`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct EventSummary {
    name: &'static str,
    token_length: Option<usize>,
    start: Option<usize>,
    end: Option<usize>,
    width: Option<usize>,
    reason: Option<&'static str>,
    pattern: Option<&'static str>,
    is_image: Option<bool>,
    result: Option<bool>,
    coupled: Option<bool>,
    has_following_colon: Option<bool>,
    follows_space_before_colon: Option<bool>,
    span_kind: Option<SpanKind>,
    fragment_kind: Option<FragmentKind>,
}

impl EventSummary {
    fn named(name: &'static str) -> Self {
        Self {
            name,
            ..Self::default()
        }
    }

    fn with_token(name: &'static str, token: &str) -> Self {
        Self {
            token_length: Some(token.chars().count()),
            ..Self::named(name)
        }
    }
}

impl From<Event<'_>> for EventSummary {
    fn from(event: Event<'_>) -> Self {
        match event {
            Event::FootnoteReferenceParsed { token } => {
                Self::with_token("FootnoteReferenceParsed", token)
            }
            Event::LinkOrImageParsed { token, is_image } => Self {
                is_image: Some(is_image),
                ..Self::with_token("LinkOrImageParsed", token)
            },
            Event::FootnoteEndNotFound { start, reason } => Self {
                start: Some(start),
                reason: Some(reason),
                ..Self::named("FootnoteEndNotFound")
            },
            Event::FootnoteLabelRecognized { start, end, token } => Self {
                start: Some(start),
                end: Some(end),
                ..Self::with_token("FootnoteLabelRecognized", token)
            },
            Event::FootnoteRefChecked { token, result } => Self {
                result: Some(result),
                ..Self::with_token("FootnoteRefChecked", token)
            },
            Event::DateSequenceMatched {
                start,
                end,
                pattern,
            } => Self {
                start: Some(start),
                end: Some(end),
                pattern: Some(pattern),
                ..Self::named("DateSequenceMatched")
            },
            Event::DateSequenceGrouped { start, end, width } => Self {
                start: Some(start),
                end: Some(end),
                width: Some(width),
                ..Self::named("DateSequenceGrouped")
            },
            Event::WhitespaceFootnoteCoupling {
                kind,
                token,
                has_following_colon,
                coupled,
            } => Self {
                span_kind: Some(kind),
                has_following_colon: Some(has_following_colon),
                coupled: Some(coupled),
                ..Self::with_token("WhitespaceFootnoteCoupling", token)
            },
            Event::FootnoteReferenceCoupling {
                kind,
                token,
                follows_space_before_colon,
                coupled,
            } => Self {
                span_kind: Some(kind),
                follows_space_before_colon: Some(follows_space_before_colon),
                coupled: Some(coupled),
                ..Self::with_token("FootnoteReferenceCoupling", token)
            },
            Event::FragmentClassified { token, kind } => Self {
                fragment_kind: Some(kind),
                ..Self::with_token("FragmentClassified", token)
            },
        }
    }
}

/// Collects events in memory so tests can assert on what the domain reported.
#[derive(Debug, Default)]
struct RecordingObserver {
    events: Vec<EventSummary>,
}

impl RecordingObserver {
    fn names(&self) -> Vec<&'static str> { self.events.iter().map(|e| e.name).collect() }

    /// Returns every recorded event with the given name.
    fn named(&self, name: &str) -> Vec<&EventSummary> {
        self.events.iter().filter(|e| e.name == name).collect()
    }
}

impl Observer for RecordingObserver {
    fn observe(&mut self, event: Event<'_>) { self.events.push(event.into()); }
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

    /// A generated link is reported as parsed with `is_image` false, and a
    /// generated image with `is_image` true, proving link and image events
    /// propagate from the tokenizer through the whole wrapping pipeline.
    ///
    /// The tokenizer's own tests call `parse_link_or_image` directly, so they
    /// would still pass if the pipeline dropped the observer on the way in.
    /// This drives `wrap_preserving_code_observed` instead, and asserts on the
    /// `is_image` field rather than the event name alone, so neither a dropped
    /// observer argument nor a mislabelled image survives.
    #[test]
    fn links_and_images_are_reported_with_their_kind(
        label in "[a-z]{1,8}",
        path in "[a-z]{1,8}",
        is_image in prop::bool::ANY,
        width in 8usize..=100,
    ) {
        let marker = if is_image { "!" } else { "" };
        let text = format!("alpha {marker}[{label}](https://example.com/{path}) omega");
        let (_, recorder) = wrap_recorded(&text, width);

        let parsed = recorder.named("LinkOrImageParsed");
        prop_assert!(
            !parsed.is_empty(),
            "expected a LinkOrImageParsed event for {text:?}, got {:?}",
            recorder.names()
        );
        prop_assert!(
            parsed.iter().any(|e| e.is_image == Some(is_image)),
            "expected is_image = {is_image} for {text:?}, got {:?}",
            parsed.iter().map(|e| e.is_image).collect::<Vec<_>>()
        );
    }

    /// A footnote-reference predicate check reports its `result`, and a
    /// coupling decision reports whether it `coupled`.
    ///
    /// These fields are booleans that no other property inspects, so this test
    /// fails if either is dropped from `EventSummary` — the omission that let
    /// `observation_is_deterministic` ignore most of each event's payload.
    #[test]
    fn boolean_decision_fields_are_reported(
        label in "[a-z]{1,8}",
        width in 8usize..=100,
    ) {
        let text = format!("alpha [^{label}]: omega");
        let (_, recorder) = wrap_recorded(&text, width);

        for name in ["FootnoteRefChecked", "WhitespaceFootnoteCoupling", "FootnoteReferenceCoupling"] {
            for event in recorder.named(name) {
                let decision = match name {
                    "FootnoteRefChecked" => event.result,
                    _ => event.coupled,
                };
                prop_assert!(
                    decision.is_some(),
                    "{name} reported no decision flag; EventSummary dropped it"
                );
            }
        }
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
