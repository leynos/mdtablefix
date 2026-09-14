//! Structured, content-free events emitted at inline grouping boundaries.

use tracing::debug;

use super::{SpanKind, predicates::looks_like_footnote_ref};

/// Record why whitespace before a footnote reference was coupled or retained.
///
/// Only token lengths and structural flags enter the event; source text stays
/// out of tracing so diagnostics cannot disclose document content.
pub(super) fn emit_whitespace_footnote_coupling(
    kind: SpanKind,
    next_token: Option<&String>,
    following_token: Option<&String>,
    coupled: bool,
) {
    let Some(token) = next_token.filter(|token| looks_like_footnote_ref(token)) else {
        return;
    };
    let token_length = token.chars().count();
    let has_following_colon = following_token.is_some_and(|following| following == ":");
    if coupled {
        debug!(
            span_kind = ?kind,
            token_length,
            has_following_colon,
            "coupled whitespace before colon-suffixed footnote reference"
        );
    } else {
        debug!(
            span_kind = ?kind,
            token_length,
            has_following_colon,
            error_category = "footnote_colon_whitespace_coupling_declined",
            "declined whitespace coupling before footnote reference"
        );
    }
}

/// Record the decision to attach an adjacent footnote reference to a span.
///
/// The colon and preceding-whitespace flags make a declined coupling
/// diagnosable without duplicating the grouping algorithm in the logger.
pub(super) fn emit_footnote_reference_coupling(
    tokens: &[String],
    end: usize,
    kind: SpanKind,
    coupled: bool,
) {
    let Some(token) = tokens
        .get(end)
        .filter(|token| looks_like_footnote_ref(token))
    else {
        return;
    };
    let token_length = token.chars().count();
    let follows_space_before_colon = end
        .checked_sub(1)
        .and_then(|previous| tokens.get(previous))
        .is_some_and(|previous| previous.chars().all(char::is_whitespace))
        && tokens
            .get(end + 1)
            .is_some_and(|following| following == ":");
    if coupled && follows_space_before_colon {
        debug!(
            span_kind = ?kind,
            token_length,
            has_following_colon = true,
            "coupled colon-suffixed footnote reference after whitespace"
        );
    } else if !coupled {
        debug!(
            span_kind = ?kind,
            token_length,
            follows_space_before_colon,
            error_category = "footnote_coupling_context_mismatch",
            "declined footnote reference coupling"
        );
    }
}
