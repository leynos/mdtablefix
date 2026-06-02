//! Paragraph wrapping utilities shared by `wrap_text`.
//!
//! These helpers keep paragraph logic focused on buffer management while
//! deferring inline wrapping to `inline::wrap_preserving_code`.

use std::borrow::Cow;

use code_span_trim::trim_code_span_edge_spaces;
use tracing::trace;
use unicode_width::UnicodeWidthStr;

use super::{inline::wrap_preserving_code, tokenize::parse_open_code_span};

mod code_span_trim;

#[cfg(test)]
#[path = "paragraph_tests.rs"]
mod tests;

/// Carries the parsed prefix metadata for a line that should be wrapped.
pub(super) struct PrefixLine<'a> {
    /// Stores the literal prefix emitted on the first wrapped line.
    pub(super) prefix: Cow<'a, str>,
    /// Stores the text that follows the prefix and should be wrapped.
    pub(super) rest: &'a str,
    /// Marks whether continuation lines should repeat the full prefix.
    pub(super) repeat_prefix: bool,
}

/// Buffers a prefixed line whose inline code span continues on later source lines.
pub(super) struct PendingPrefix {
    /// Stores the bullet/blockquote/footnote marker plus any leading indent.
    pub(super) prefix: String,
    /// Stores the line content after the prefix, including any open code span.
    pub(super) rest: String,
    /// Stores the original source lines when unsafe continuations need passthrough.
    pub(super) original_lines: Vec<String>,
    /// Byte offsets of spaces inserted while joining continuation lines.
    pub(super) synthetic_join_spaces: Vec<usize>,
    /// Stores the precomputed content width available on the first line.
    pub(super) rest_width: usize,
    /// Marks whether continuation lines should repeat the full prefix.
    pub(super) repeat_prefix: bool,
    /// Marks whether the closing continuation ended with a Markdown hard break.
    pub(super) hard_break: bool,
    /// Fence length of the inline code span that is currently open, if any.
    pub(super) open_fence_len: Option<usize>,
    /// Controls how continuation chunks are joined and flushed.
    pub(super) continuation_mode: ContinuationMode,
    /// Marks whether the original prefix has already been emitted.
    pub(super) used_prefix: bool,
}

/// Controls how a pending prefixed continuation should be joined or emitted.
#[derive(Debug, PartialEq)]
pub(super) enum ContinuationMode {
    /// Join continuations using normal Markdown soft-break spacing.
    Normalize,
    /// Join without adding a synthetic space after an opener at EOL.
    TightCodeSpan,
    /// Emit the original source lines instead of rewrapping ambiguous input.
    VerbatimFlush,
}

/// Tracks buffered paragraph content and its shared indentation.
#[derive(Default)]
pub(super) struct ParagraphState {
    /// Stores buffered paragraph segments and whether each ends with a hard break.
    buf: Vec<(String, bool)>,
    /// Stores the leading indentation reused for wrapped continuation lines.
    indent: String,
    /// Stores the list continuation indent after a deferred prefix flush.
    continuation_indent: Option<String>,
    /// Stores a prefixed line waiting for a cross-line code span to close.
    pub(super) pending_prefix: Option<PendingPrefix>,
}

impl ParagraphState {
    /// Clears the buffered paragraph state.
    ///
    /// This resets both the accumulated segments and the remembered indent.
    /// It returns no value and preserves the invariant that an empty state has
    /// no buffered text. This method never panics.
    pub(super) fn clear(&mut self) {
        self.buf.clear();
        self.indent.clear();
        self.continuation_indent = None;
        self.pending_prefix = None;
    }

    /// Records the paragraph indent from `line` when the buffer is still empty.
    ///
    /// The `line` parameter is the original input line whose leading
    /// whitespace may become the continuation prefix. This method returns no
    /// value, updates `indent` only for the first buffered segment, and never
    /// panics.
    pub(super) fn note_indent(&mut self, line: &str) {
        if self.buf.is_empty() {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            if indent.is_empty() {
                self.continuation_indent = None;
                self.indent.clear();
            } else {
                self.indent = self.continuation_indent.take().unwrap_or(indent);
            }
        }
    }

    /// Records the continuation indent for the next indented paragraph.
    pub(super) fn remember_continuation_indent(&mut self, indent: String) {
        self.continuation_indent = Some(indent);
    }

    /// Appends one paragraph segment and its hard-break marker.
    ///
    /// `text` is stored verbatim and `hard_break` records whether the source
    /// line ended with Markdown hard-break spacing. This method returns no
    /// value and keeps buffered segments in input order without panicking.
    pub(super) fn push(&mut self, text: String, hard_break: bool) {
        self.buf.push((text, hard_break));
    }

    /// Takes the deferred prefixed segment and resets plain paragraph buffers.
    ///
    /// This keeps the pending-prefix state transition separate from output
    /// emission. It returns the pending prefix when one exists, clears the
    /// regular paragraph buffer and indent, and leaves `pending_prefix` empty.
    pub(super) fn drain_pending_prefix(&mut self) -> Option<PendingPrefix> {
        let pending = self.pending_prefix.take()?;
        self.buf.clear();
        self.indent.clear();
        Some(pending)
    }
}

/// Emits wrapped paragraph lines into the caller-provided output buffer.
pub(super) struct ParagraphWriter<'a> {
    /// Borrows the caller-owned output buffer that receives emitted lines.
    out: &'a mut Vec<String>,
    /// Stores the target wrap width in Unicode display columns.
    width: usize,
}

impl<'a> ParagraphWriter<'a> {
    /// Creates a writer over `out` with the target wrap `width`.
    ///
    /// The returned writer borrows `out` for subsequent emission. `width` is
    /// interpreted in Unicode display columns, and the constructor never
    /// panics.
    pub(super) fn new(out: &'a mut Vec<String>, width: usize) -> Self { Self { out, width } }

    /// Wraps `text` with `prefix` on the first line and `continuation_prefix`
    /// on later lines.
    ///
    /// The available content width is computed once from the Unicode display
    /// width of `prefix`. This method returns no value, emits directly into
    /// `out`, and preserves the invariant that every continuation line uses
    /// the supplied continuation prefix.
    fn wrap_with_prefix(&mut self, prefix: &str, continuation_prefix: &str, text: &str) {
        let prefix_width = UnicodeWidthStr::width(prefix);
        let available = self.width.saturating_sub(prefix_width).max(1);
        let lines = wrap_preserving_code(text, available);
        if lines.is_empty() {
            self.out.push(prefix.to_string());
            return;
        }

        for (index, wrapped_line) in lines.iter().enumerate() {
            if index == 0 {
                self.out.push(format!("{prefix}{wrapped_line}"));
            } else {
                self.out
                    .push(format!("{continuation_prefix}{wrapped_line}"));
            }
        }
    }

    /// Wraps one parsed prefixed line using the correct continuation prefix.
    ///
    /// `line` supplies the first-line prefix, the text to wrap, and whether
    /// the full prefix should repeat on continuations. This method returns no
    /// value and keeps continuation alignment in the same visual column.
    fn append_wrapped_with_prefix(&mut self, line: &PrefixLine<'_>) {
        let prefix = line.prefix.as_ref();
        let prefix_width = UnicodeWidthStr::width(prefix);
        let available = self.width.saturating_sub(prefix_width).max(1);
        self.append_wrapped_with_prefix_width(line, available);
    }

    pub(super) fn append_wrapped_with_prefix_width(
        &mut self,
        line: &PrefixLine<'_>,
        available: usize,
    ) {
        let prefix = line.prefix.as_ref();
        let continuation_prefix = continuation_prefix_for(prefix, line.repeat_prefix);

        let lines = wrap_preserving_code(line.rest, available);
        if lines.is_empty() {
            self.out.push(prefix.to_string());
            return;
        }

        for (index, wrapped_line) in lines.iter().enumerate() {
            if index == 0 {
                self.out.push(format!("{prefix}{wrapped_line}"));
            } else {
                self.out
                    .push(format!("{continuation_prefix}{wrapped_line}"));
            }
        }
    }

    pub(super) fn ensure_trailing_hard_break_on_last_line(&mut self) {
        if let Some(last) = self.out.last_mut()
            && !last.ends_with("  ")
        {
            last.push_str("  ");
        }
    }

    /// Emits a buffered prefix line and its verbatim continuation.
    ///
    /// `pending` provides the stored prefix, the original first-line text,
    /// and whether the prefix must repeat on continuations. `continuation`
    /// is the original source continuation line, and `hard_break` applies
    /// trailing Markdown hard-break spacing when needed. This method writes
    /// two lines to `out` and leaves the buffered state untouched.
    pub(super) fn emit_pending_with_verbatim_continuation(
        &mut self,
        pending: PendingPrefix,
        continuation: &str,
        hard_break: bool,
    ) {
        let prefix = pending.prefix;
        let mut first_line = format!("{prefix}{rest}", rest = pending.rest);
        if pending.hard_break && !first_line.ends_with("  ") {
            first_line.push_str("  ");
        }
        self.out.push(first_line);

        let mut continuation_line = continuation.to_string();
        if hard_break && !continuation_line.ends_with("  ") {
            continuation_line.push_str("  ");
        }
        self.out.push(continuation_line);
    }

    /// Flushes the buffered paragraph into wrapped output lines.
    ///
    /// `state` supplies the buffered segments and remembered indent. This
    /// method returns no value, clears the state when flushing completes, and
    /// preserves hard-break segments as distinct wrapped emissions.
    pub(super) fn flush_paragraph(&mut self, state: &mut ParagraphState) {
        if let Some(pending) = state.drain_pending_prefix() {
            let mut pending = pending;

            if pending.continuation_mode == ContinuationMode::VerbatimFlush {
                if !pending.repeat_prefix {
                    state.remember_continuation_indent(continuation_prefix_for(
                        pending.prefix.as_str(),
                        pending.repeat_prefix,
                    ));
                }
                self.out.extend(pending.original_lines);
                return;
            }

            // Advances `used_prefix` so a final flush never repeats list markers.
            let prefix = pending_prefix_for_next_segment(&mut pending);
            let rest = trim_code_span_edge_spaces(&pending.rest, &pending.synthetic_join_spaces);
            let prefix_line = PrefixLine {
                prefix: Cow::Owned(prefix),
                rest: rest.as_ref(),
                repeat_prefix: pending.repeat_prefix,
            };
            self.append_wrapped_with_prefix_width(&prefix_line, pending.rest_width);
            if pending.hard_break {
                self.ensure_trailing_hard_break_on_last_line();
            }
            if !pending.repeat_prefix {
                state.remember_continuation_indent(continuation_prefix_for(
                    pending.prefix.as_str(),
                    pending.repeat_prefix,
                ));
            }
        }

        if state.buf.is_empty() {
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
                segment.clear();
            }
        }

        if !segment.is_empty() {
            self.push_wrapped_segment(&state.indent, &segment);
        }

        state.clear();
    }

    /// Wraps one buffered `segment` using the shared indentation `indent`.
    ///
    /// Both parameters are emitted through `wrap_with_prefix`, and the method
    /// returns no value. It assumes `indent` already reflects the paragraph's
    /// visual indent and never panics.
    fn push_wrapped_segment(&mut self, indent: &str, segment: &str) {
        self.wrap_with_prefix(indent, indent, segment);
    }

    /// Flushes any active paragraph and then emits `line` verbatim.
    ///
    /// `state` is flushed before `line` is appended unchanged to `out`. This
    /// method returns no value and preserves the invariant that verbatim lines
    /// break paragraph accumulation boundaries.
    pub(super) fn push_verbatim(&mut self, state: &mut ParagraphState, line: &str) {
        self.flush_paragraph(state);
        state.continuation_indent = None;
        self.out.push(line.to_string());
    }

    /// Flushes any active paragraph and wraps `prefix_line`.
    ///
    /// `state` is flushed first so prefixed lines never join the preceding
    /// paragraph buffer. This method returns no value and emits the wrapped
    /// output directly into `out`.
    pub(super) fn handle_prefix_line(
        &mut self,
        state: &mut ParagraphState,
        prefix_line: &PrefixLine<'_>,
    ) {
        self.flush_paragraph(state);
        state.continuation_indent = None;

        if let Some((fence_len, open_tail)) = parse_open_code_span(prefix_line.rest) {
            let prefix = prefix_line.prefix.as_ref().to_string();
            let prefix_width = UnicodeWidthStr::width(prefix.as_str());
            let opener_at_eol = open_tail.trim().is_empty();
            let continuation_mode = if opener_at_eol {
                ContinuationMode::TightCodeSpan
            } else {
                ContinuationMode::Normalize
            };
            trace!(
                ?continuation_mode,
                opener_at_eol, fence_len, "selected pending-prefix continuation mode"
            );
            state.pending_prefix = Some(PendingPrefix {
                prefix,
                rest: prefix_line.rest.to_string(),
                original_lines: vec![format!(
                    "{prefix}{rest}",
                    prefix = prefix_line.prefix.as_ref(),
                    rest = prefix_line.rest,
                )],
                synthetic_join_spaces: Vec::new(),
                rest_width: self.width.saturating_sub(prefix_width).max(1),
                repeat_prefix: prefix_line.repeat_prefix,
                hard_break: false,
                open_fence_len: Some(fence_len),
                continuation_mode,
                used_prefix: false,
            });
            return;
        }

        self.append_wrapped_with_prefix(prefix_line);
    }
}

pub(super) fn pending_prefix_for_next_segment(pending: &mut PendingPrefix) -> String {
    if pending.used_prefix {
        continuation_prefix_for(pending.prefix.as_str(), pending.repeat_prefix)
    } else {
        pending.used_prefix = true;
        pending.prefix.clone()
    }
}

fn continuation_prefix_for(prefix: &str, repeat_prefix: bool) -> String {
    if repeat_prefix {
        return prefix.to_string();
    }

    let prefix_width = UnicodeWidthStr::width(prefix);
    let indent_str: String = prefix.chars().take_while(|c| c.is_whitespace()).collect();
    let indent_width = UnicodeWidthStr::width(indent_str.as_str());
    format!("{}{}", indent_str, " ".repeat(prefix_width - indent_width))
}
