//! Stable tail-reflow and hard-break helpers for `ParagraphWriter`.
//!
//! These helpers keep the deferred prefixed-span flush deterministic by
//! rewrapping the tail independently and preserving Markdown hard-break
//! markers, isolating that logic from the buffer-management code in the parent
//! module.

use super::{
    ParagraphWriter,
    PrefixLine,
    continuation_prefix_for,
    hard_break::trailing_hard_break_marker_len,
    wrap_preserving_code,
};

impl ParagraphWriter<'_> {
    /// Intentionally splits in two stages and rewraps the tail independently,
    /// making each flush deterministic and idempotent for the same prefix,
    /// rest, and available width. Do not replace this with the single-pass
    /// approach used by `append_wrapped_with_prefix_width`.
    pub(super) fn append_stable_pending_prefix(&mut self, line: &PrefixLine<'_>, available: usize) {
        if line.repeat_prefix {
            self.append_wrapped_with_prefix_width(line, available);
            return;
        }

        let mut lines = wrap_preserving_code(line.rest, available).into_iter();
        let Some(first_line) = lines.next() else {
            self.out.push(line.prefix.to_string());
            return;
        };
        self.out.push(format!("{}{first_line}", line.prefix));

        let continuation_prefix =
            continuation_prefix_for(line.prefix.as_ref(), false, line.outer_prefix.as_deref());
        let mut tail_segment = String::new();
        for wrapped_line in lines {
            let marker_len = trailing_hard_break_marker_len(&wrapped_line);
            let content_end = wrapped_line.len() - marker_len;
            if !tail_segment.is_empty() {
                tail_segment.push(' ');
            }
            tail_segment.push_str(&wrapped_line[..content_end]);

            if marker_len > 0 {
                self.emit_tail_segment(
                    &mut tail_segment,
                    &wrapped_line[content_end..],
                    continuation_prefix.as_str(),
                    available,
                );
            }
        }
        self.emit_tail_segment(&mut tail_segment, "", &continuation_prefix, available);
    }

    fn emit_tail_segment(
        &mut self,
        tail_segment: &mut String,
        marker: &str,
        continuation_prefix: &str,
        available: usize,
    ) {
        self.out.extend(
            wrap_preserving_code(tail_segment, available)
                .into_iter()
                .map(|tail_line| format!("{continuation_prefix}{tail_line}")),
        );
        if let Some(last_line) = self.out.last_mut() {
            last_line.push_str(marker);
        }
        tail_segment.clear();
    }

    /// Appends a two-space hard-break marker to the last emitted line,
    /// mutating it in place rather than pushing a new line. Does nothing if
    /// `out` is empty. An odd trailing-backslash run is treated as an
    /// existing hard break (via `trailing_hard_break_marker_len`), so such a
    /// line is left untouched.
    pub(in crate::wrap) fn ensure_trailing_hard_break_on_last_line(&mut self) {
        if let Some(last) = self.out.last_mut()
            && trailing_hard_break_marker_len(last) == 0
        {
            last.push_str("  ");
        }
    }
}
