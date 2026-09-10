//! Resolution and dispatch for pending prefixed continuations.
//!
//! These helpers decide whether the next source line continues a buffered
//! prefix or flushes it, keeping the pending-prefix branch of
//! `dispatch_continuation` out of `wrap.rs`.

use tracing::trace;

use super::{
    LineContext,
    block::BlockKind,
    continuation::apply_continuation_chunk,
    line_break_parts,
    link_reference::{self, LinkReferenceMatcher},
    paragraph::{
        ParagraphState,
        ParagraphWriter,
        PendingPrefix,
        continuation_folds_tail,
        continuation_prefix_for,
        wraps_to_tail,
    },
    prefix_line,
    try_blockquote_fast_path,
    try_passthrough_block,
};

/// Returns the source text that continues the pending prefix from `line`.
///
/// A line carrying the block's continuation prefix continues it directly. A
/// line without that prefix is a lazy continuation, which Markdown folds into
/// the open block; it is folded here when the block emits a tail line that
/// absorbs the lines below it, matching how the block's own output re-parses on
/// the next pass. Otherwise the source line stays a separate paragraph on both
/// passes and `None` is returned so the caller flushes the block first.
fn pending_continuation_text<'a>(
    pending: &PendingPrefix,
    line: LineContext<'a>,
) -> Option<&'a str> {
    if pending.open_fence_len.is_some() {
        return Some(line.inner);
    }

    let prefix = continuation_prefix_for(
        pending.prefix.as_str(),
        pending.repeat_prefix,
        pending.outer_prefix.as_deref(),
    );
    if let Some(continuation) = line.original.strip_prefix(prefix.as_str()) {
        return Some(continuation);
    }

    let absorbs_following_lines = continuation_folds_tail(prefix.as_str())
        && wraps_to_tail(pending.rest.as_str(), pending.rest_width);
    if absorbs_following_lines {
        trace!(
            mode = "pending_prefix",
            boundary = "prefix_mismatch",
            line_len = line.original.len(),
            "joining a lazy continuation after its prefix changed"
        );
        return Some(line.inner);
    }

    trace!(
        mode = "pending_prefix",
        boundary = "prefix_mismatch",
        line_len = line.original.len(),
        "flushing a pending continuation after its prefix changed"
    );
    None
}

fn resolve_or_fallback_continuation(
    line: LineContext<'_>,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
) -> bool {
    let continuation = state
        .pending_prefix
        .as_ref()
        .and_then(|pending| pending_continuation_text(pending, line));
    let Some(continuation) = continuation else {
        writer.flush_paragraph(state);
        return false;
    };

    let (text, hard_break) = line_break_parts(continuation);
    apply_continuation_chunk(&text, line.original, hard_break, writer, state);
    true
}

pub(super) fn handle_pending_continuation(
    line: LineContext<'_>,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
    link_matcher: LinkReferenceMatcher,
    link_title_window: &mut link_reference::LinkTitleWindow,
) -> bool {
    if try_blockquote_fast_path(line, writer, state) {
        return true;
    }

    // A thematic break never continues an open prefixed span. Without this
    // guard a spaced run such as `- - -` would fall through to the bullet
    // branch below, which matches it as a list item and absorbs the break.
    if line.block_kind == Some(BlockKind::ThematicBreak) {
        return try_passthrough_block(line, writer, state, link_matcher, link_title_window);
    }

    if let Some(prefix_line) = prefix_line(line.inner, line.blockquote) {
        let matches_pending = state.pending_prefix.as_ref().is_some_and(|pending| {
            prefix_line.repeat_prefix && pending.prefix == prefix_line.prefix.as_ref()
        });
        if matches_pending {
            let (text, hard_break) = line_break_parts(prefix_line.rest);
            apply_continuation_chunk(&text, line.original, hard_break, writer, state);
            return true;
        }

        writer.handle_prefix_line(state, &prefix_line);
        return true;
    }

    if try_passthrough_block(line, writer, state, link_matcher, link_title_window) {
        return true;
    }

    resolve_or_fallback_continuation(line, writer, state)
}
