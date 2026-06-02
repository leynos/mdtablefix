//! Unit tests for pending-prefix continuation decisions.

use std::borrow::Cow;

use super::*;

fn pending_prefix(
    continuation_mode: ContinuationMode,
    rest: &str,
    rest_width: usize,
) -> PendingPrefix {
    PendingPrefix {
        prefix: "- ".to_string(),
        rest: rest.to_string(),
        original_lines: vec![format!("- {rest}")],
        synthetic_join_spaces: Vec::new(),
        rest_width,
        repeat_prefix: false,
        hard_break: false,
        open_fence_len: Some(1),
        continuation_mode,
    }
}

fn state_with_pending(pending: PendingPrefix) -> ParagraphState {
    let mut state = ParagraphState::default();
    state.pending_prefix = Some(pending);
    state
}

#[test]
fn should_emit_verbatim_for_width_only_when_join_would_overflow() {
    let rest = "`EngineConnector::connect(socket: impl AsRef<str>)";
    let text = " -> Result<Docker, PodbotError>`";
    let overflowing = state_with_pending(pending_prefix(ContinuationMode::Normalize, rest, 40));
    let fitting = state_with_pending(pending_prefix(ContinuationMode::Normalize, rest, 120));

    assert!(should_emit_verbatim_for_width(text, &overflowing));
    assert!(!should_emit_verbatim_for_width(text, &fitting));
}

#[test]
fn apply_continuation_chunk_emits_overwidth_continuation_verbatim() {
    let mut out = Vec::new();
    let mut writer = ParagraphWriter::new(&mut out, 80);
    let rest = "`EngineConnector::connect(socket: impl AsRef<str>)";
    let mut state = state_with_pending(pending_prefix(ContinuationMode::Normalize, rest, 40));

    apply_continuation_chunk(
        " -> Result<Docker, PodbotError>`",
        "  -> Result<Docker, PodbotError>`",
        false,
        &mut writer,
        &mut state,
    );

    assert_eq!(
        out,
        vec![
            "- `EngineConnector::connect(socket: impl AsRef<str>)",
            "  -> Result<Docker, PodbotError>`",
        ]
    );
    assert!(state.pending_prefix.is_none());
}

#[test]
fn apply_continuation_chunk_preserves_original_lines_for_verbatim_flush() {
    let mut out = Vec::new();
    let mut writer = ParagraphWriter::new(&mut out, 80);
    let mut pending = pending_prefix(ContinuationMode::VerbatimFlush, "rewritten `a", 3);
    pending.original_lines = vec!["- original `a".to_string()];
    let mut state = state_with_pending(pending);

    apply_continuation_chunk("b`", "  b`", false, &mut writer, &mut state);

    assert_eq!(out, vec!["- original `a", "  b`"]);
    assert!(state.pending_prefix.is_none());
}

#[test]
fn handle_prefix_line_selects_tight_mode_for_opener_at_eol() {
    let mut out = Vec::new();
    let mut writer = ParagraphWriter::new(&mut out, 80);
    let mut state = ParagraphState::default();
    let line = PrefixLine {
        prefix: Cow::Borrowed("- "),
        rest: "run `",
        repeat_prefix: false,
    };

    writer.handle_prefix_line(&mut state, &line);

    assert!(matches!(
        state
            .pending_prefix
            .map(|pending| pending.continuation_mode),
        Some(ContinuationMode::TightCodeSpan)
    ));
}

#[test]
fn handle_prefix_line_selects_normal_mode_for_nonempty_tail() {
    let mut out = Vec::new();
    let mut writer = ParagraphWriter::new(&mut out, 80);
    let mut state = ParagraphState::default();
    let line = PrefixLine {
        prefix: Cow::Borrowed("- "),
        rest: "run `code",
        repeat_prefix: false,
    };

    writer.handle_prefix_line(&mut state, &line);

    assert!(matches!(
        state
            .pending_prefix
            .map(|pending| pending.continuation_mode),
        Some(ContinuationMode::Normalize)
    ));
}

#[test]
fn update_span_state_selects_verbatim_flush_for_word_tail_after_close() {
    let mut pending = pending_prefix(ContinuationMode::Normalize, "`foo", 80);

    let update = update_span_state("bar`baz", 0, &mut pending);

    assert!(matches!(update, SpanStateUpdate::Flush));
    assert!(matches!(
        pending.continuation_mode,
        ContinuationMode::VerbatimFlush
    ));
}
