//! Coupling reports for inline span grouping.
//!
//! Grouping decides which tokens are joined into one atomic span. Some of those
//! decisions carry diagnostic value, and this module is where the decision is
//! turned into an [`Event`] for the observer. Keeping it apart from the grouping
//! rules means the rules read as policy, and the reporting reads as the
//! translation from policy to diagnostics.
//!
//! Every emitter is a no-op when no observer is attached, so reporting never
//! changes grouping or layout.

use super::{predicates::looks_like_footnote_ref, span_grouping::CouplingReport};
use crate::wrap::observer::{Event, ObserverHandle};

/// Reports whether whitespace before a colon-suffixed footnote reference was
/// coupled into the current span.
///
/// Nothing is reported unless the next token really is a footnote reference, so
/// the event describes a grouping decision rather than every whitespace run.
/// The probe passes `&mut None`: it only selects whether to report, and
/// forwarding the observer would emit a `FootnoteRefChecked` event for it.
pub(super) fn emit_whitespace_footnote_coupling(
    report: CouplingReport<'_>,
    observer: &mut ObserverHandle<'_>,
) {
    // Nothing will consume the event, so skip the probe entirely rather than
    // paying for it and discarding the result.
    let Some(observer) = observer.as_deref_mut() else {
        return;
    };
    let Some(token) = report
        .tokens
        .get(report.end + 1)
        .filter(|token| looks_like_footnote_ref(token, &mut None))
    else {
        return;
    };
    observer.observe(Event::WhitespaceFootnoteCoupling {
        kind: report.kind,
        token,
        has_following_colon: report
            .tokens
            .get(report.end + 2)
            .is_some_and(|following| following == ":"),
        coupled: report.coupled,
    });
}

/// Reports whether an adjacent footnote reference was coupled into the current
/// span, and the context that decided it.
pub(super) fn emit_footnote_reference_coupling(
    report: CouplingReport<'_>,
    observer: &mut ObserverHandle<'_>,
) {
    // Nothing will consume the event, so skip the probe and the coupling
    // context rather than computing them and discarding the result.
    let Some(observer) = observer.as_deref_mut() else {
        return;
    };
    let Some(token) = report
        .tokens
        .get(report.end)
        .filter(|token| looks_like_footnote_ref(token, &mut None))
    else {
        return;
    };
    let follows_space_before_colon = report
        .end
        .checked_sub(1)
        .and_then(|previous| report.tokens.get(previous))
        .is_some_and(|previous| previous.chars().all(char::is_whitespace))
        && report
            .tokens
            .get(report.end + 1)
            .is_some_and(|following| following == ":");
    observer.observe(Event::FootnoteReferenceCoupling {
        kind: report.kind,
        token,
        follows_space_before_colon,
        coupled: report.coupled,
    });
}
