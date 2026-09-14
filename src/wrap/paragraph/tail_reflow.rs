//! Stable tail-reflow and hard-break helpers for `ParagraphWriter`.
//!
//! These helpers keep the deferred prefixed-span flush deterministic by
//! rewrapping the tail independently and preserving Markdown hard-break
//! markers, isolating that logic from the buffer-management code in the parent
//! module.

use unicode_width::UnicodeWidthStr;

use super::{
    ParagraphWriter,
    PrefixLine,
    continuation_folds_tail,
    continuation_prefix_for,
    hard_break::trailing_hard_break_marker_len,
    wrap_preserving_code,
    wraps_to_tail,
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
            let marker = &wrapped_line[content_end..];
            if !tail_segment.is_empty() {
                tail_segment.push(' ');
            }
            tail_segment.push_str(&wrapped_line[..content_end]);

            if marker_len > 0 {
                // A backslash hard break is content: it stays glued to the last
                // word of the source line, so the wrap must measure it. Stripping
                // it filled the last line to `available` and then appended the
                // backslash past the budget, and the next pass — which measures
                // it — re-wrapped that line one word earlier. A whitespace marker
                // is different: the next pass trims it before measuring, so it is
                // stripped here and appended afterwards, keeping it out of the
                // fit exactly as the re-read does.
                let (measured, appended) = if marker.trim().is_empty() {
                    ("", marker)
                } else {
                    (marker, "")
                };
                tail_segment.push_str(measured);
                self.emit_tail_segment(
                    &mut tail_segment,
                    appended,
                    continuation_prefix.as_str(),
                    available,
                );
            }
        }
        self.emit_tail_segment(&mut tail_segment, "", &continuation_prefix, available);
    }

    /// Rewrap one deferred tail and apply its hard-break marker to the last
    /// emitted line.
    ///
    /// `tail_segment` is cleared after emission so each source hard break is
    /// handled independently; the continuation prefix is applied only to the
    /// lines produced for this segment.
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

    /// Returns whether `line` must be deferred so its tail cannot absorb later lines.
    ///
    /// A prefixed source line that fits on a single output line emits no tail,
    /// so the following source lines are re-wrapped as a fresh paragraph on
    /// both this pass and the next: the result is already a fixed point. The
    /// same holds when the tail does not re-parse as paragraph text; see
    /// [`continuation_folds_tail`].
    ///
    /// Once a line that folds its tail spills onto one, the next pass joins the
    /// following source lines into that tail, so the first pass must join them
    /// too. Deferring the line lets the buffered continuation lines be reflowed
    /// with it, emitting what the next pass would.
    pub(super) fn prefix_line_needs_tail_deferral(&self, line: &PrefixLine<'_>) -> bool {
        let continuation_prefix = continuation_prefix_for(
            line.prefix.as_ref(),
            line.repeat_prefix,
            line.outer_prefix.as_deref(),
        );
        if !continuation_folds_tail(continuation_prefix.as_str()) {
            return false;
        }
        let prefix_width = UnicodeWidthStr::width(line.prefix.as_ref());
        let available = self.width.saturating_sub(prefix_width).max(1);
        wraps_to_tail(line.rest, available)
    }
}
