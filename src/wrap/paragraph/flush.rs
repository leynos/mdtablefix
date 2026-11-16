//! Flushing of a buffered paragraph and any pending prefix line into wrapped output.
//!
//! These steps back [`ParagraphWriter::flush_paragraph`]: the pending prefix line is
//! flushed first, then the buffered segments, so the paragraph reads in source order.

use std::borrow::Cow;

use super::{
    ContinuationMode,
    ParagraphState,
    ParagraphWriter,
    PendingPrefix,
    PrefixLine,
    continuation_prefix_for,
    pending_prefix_for_next_segment,
    spanning_code,
    trim_code_span_edge_spaces,
};

impl ParagraphWriter<'_> {
    /// Flushes a drained pending prefix line, returning `true` when it was emitted verbatim.
    ///
    /// A verbatim flush emits the original source lines and completes the whole paragraph
    /// flush, so the caller must not flush the buffered segments afterwards.
    pub(super) fn flush_pending_prefix(
        &mut self,
        state: &mut ParagraphState,
        mut pending: PendingPrefix,
    ) -> bool {
        if pending.continuation_mode == ContinuationMode::VerbatimFlush {
            if !pending.repeat_prefix {
                remember_pending_indent(state, &pending);
            }
            self.out.extend(pending.original_lines);
            return true;
        }

        // Advances `used_prefix` so a final flush never repeats list markers.
        let prefix = pending_prefix_for_next_segment(&mut pending);
        let rest = trim_code_span_edge_spaces(&pending.rest, &pending.synthetic_join_spaces);
        let prefix_line = PrefixLine {
            prefix: Cow::Owned(prefix),
            rest: rest.as_ref(),
            repeat_prefix: pending.repeat_prefix,
            outer_prefix: pending.outer_prefix.as_deref().map(Cow::Borrowed),
        };
        self.append_stable_pending_prefix(&prefix_line, pending.rest_width);
        if pending.hard_break {
            self.ensure_trailing_hard_break_on_last_line();
        }
        if !pending.repeat_prefix {
            remember_pending_indent(state, &pending);
        }
        false
    }

    /// Wraps the buffered segments, breaking at hard breaks, and clears the buffer.
    pub(super) fn flush_buffered_segments(&mut self, state: &mut ParagraphState) {
        if state.buf.is_empty() {
            return;
        }

        if let Some(lines) = spanning_code::conforming_source_lines_for_overlong_span(
            &state.buf,
            &state.indent,
            self.width,
        ) {
            self.out.extend(lines);
            state.clear();
            return;
        }

        let mut segment = String::new();
        for (text, hard_break) in &state.buf {
            if !segment.is_empty() {
                segment.push(' ');
            }
            segment.push_str(text);
            if *hard_break {
                self.push_wrapped_segment(&state.indent, &segment);
                self.ensure_trailing_hard_break_on_last_line();
                segment.clear();
            }
        }

        if !segment.is_empty() {
            self.push_wrapped_segment(&state.indent, &segment);
        }

        state.clear();
    }
}

/// Remembers the continuation indent that follows a pending prefix line.
fn remember_pending_indent(state: &mut ParagraphState, pending: &PendingPrefix) {
    state.remember_continuation_indent(continuation_prefix_for(
        pending.prefix.as_str(),
        pending.repeat_prefix,
        pending.outer_prefix.as_deref(),
    ));
}
