//! Paragraph wrapping utilities shared by `wrap_text`.
//!
//! These helpers keep paragraph logic focused on buffer management while
//! deferring inline wrapping to `inline::wrap_preserving_code`.

use std::borrow::Cow;

use unicode_width::UnicodeWidthStr;

use super::inline::wrap_preserving_code;

/// A prefixed line from the input stream such as a bullet, blockquote,
/// footnote, or checkbox entry.
///
/// `prefix` holds the leading marker and any associated whitespace.
/// `rest` is the content after that prefix. `repeat_prefix` controls
/// whether continuation lines use the same prefix string (`true`) or a
/// space-padded equivalent of the same display width (`false`).
pub(super) struct PrefixLine<'a> {
    /// The leading marker string (e.g. `"> "`, `"- "`, `"[^1]: "`).
    pub(super) prefix: Cow<'a, str>,
    /// The text content after the prefix.
    pub(super) rest: &'a str,
    /// When `true`, continuation lines repeat `prefix` verbatim (blockquotes).
    /// When `false`, continuation lines use a space-padded indent of the same
    /// display width (bullets, footnotes).
    pub(super) repeat_prefix: bool,
}

/// Accumulates buffered paragraph lines before they are flushed and wrapped.
///
/// `ParagraphState` collects consecutive plain-text lines into a single
/// segment so that `ParagraphWriter::flush_paragraph` can wrap the entire
/// paragraph as one unit, respecting hard line breaks (two trailing spaces).
#[derive(Default)]
pub(super) struct ParagraphState {
    /// Buffered text segments with a flag indicating whether each ends with a
    /// Markdown hard line break.
    buf: Vec<(String, bool)>,
    /// The leading whitespace indent detected from the first buffered line.
    indent: String,
}

impl ParagraphState {
    /// Clears all buffered segments and resets the indent.
    pub(super) fn clear(&mut self) {
        self.buf.clear();
        self.indent.clear();
    }

    /// Records the leading whitespace indent from `line` if this is the first
    /// line pushed into the buffer.
    pub(super) fn note_indent(&mut self, line: &str) {
        if self.buf.is_empty() {
            self.indent = line.chars().take_while(|c| c.is_whitespace()).collect();
        }
    }

    /// Appends a text segment to the buffer.
    ///
    /// `hard_break` should be `true` when the source line ends with two or
    /// more trailing spaces, signalling a Markdown hard line break.
    pub(super) fn push(&mut self, text: String, hard_break: bool) {
        self.buf.push((text, hard_break));
    }
}

/// Writes wrapped output lines to an output `Vec<String>`.
///
/// `ParagraphWriter` centralises prefix-aware and plain-paragraph wrapping so
/// that `wrap_text` does not need to manage display-width calculations or
/// continuation-prefix logic directly.
pub(super) struct ParagraphWriter<'a> {
    /// The output buffer that receives wrapped lines.
    out: &'a mut Vec<String>,
    /// Target line width in display columns.
    width: usize,
}

impl<'a> ParagraphWriter<'a> {
    /// Creates a new `ParagraphWriter` that appends wrapped lines to `out`
    /// with a target display width of `width` columns.
    pub(super) fn new(out: &'a mut Vec<String>, width: usize) -> Self { Self { out, width } }

    /// Wraps `text` to fit within `self.width` minus the display width of
    /// `prefix`, then emits the first wrapped line as `{prefix}{line}` and
    /// all subsequent lines as `{continuation_prefix}{line}`.
    ///
    /// When `wrap_preserving_code` returns no lines (empty `text`), a single
    /// line consisting of `prefix` alone is emitted.
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

    /// Derives the continuation prefix for a prefixed line and delegates to
    /// `wrap_with_prefix`.
    ///
    /// When `line.repeat_prefix` is `true` (blockquotes), the same prefix is
    /// used on all lines. Otherwise a space-padded string of the same display
    /// width is used so that continuation content aligns with the first line's
    /// text column.
    fn append_wrapped_with_prefix(&mut self, line: &PrefixLine<'_>) {
        let prefix = line.prefix.as_ref();
        let prefix_width = UnicodeWidthStr::width(prefix);
        let indent_str: String = prefix.chars().take_while(|c| c.is_whitespace()).collect();
        let indent_width = UnicodeWidthStr::width(indent_str.as_str());
        let continuation_prefix = if line.repeat_prefix {
            prefix.to_string()
        } else {
            format!("{}{}", indent_str, " ".repeat(prefix_width - indent_width))
        };

        self.wrap_with_prefix(prefix, continuation_prefix.as_str(), line.rest);
    }

    /// Flushes all buffered paragraph segments in `state` as wrapped output.
    ///
    /// Consecutive segments are joined with a single space before wrapping.
    /// When a segment ends with a hard line break flag, it is wrapped
    /// independently and the accumulation resets, preserving the Markdown
    /// hard-break semantics. The state is cleared after flushing.
    pub(super) fn flush_paragraph(&mut self, state: &mut ParagraphState) {
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

    /// Wraps a single `segment` with `indent` applied as both the first-line
    /// prefix and the continuation prefix.
    fn push_wrapped_segment(&mut self, indent: &str, segment: &str) {
        self.wrap_with_prefix(indent, indent, segment);
    }

    /// Flushes any buffered paragraph and then appends `line` verbatim to the
    /// output without wrapping.
    ///
    /// Used for lines that must not be reflowed (fenced code blocks, tables,
    /// headings, and Markdownlint directives).
    pub(super) fn push_verbatim(&mut self, state: &mut ParagraphState, line: &str) {
        self.flush_paragraph(state);
        self.out.push(line.to_string());
    }

    /// Flushes any buffered paragraph and then wraps `prefix_line` using
    /// `append_wrapped_with_prefix`.
    ///
    /// Called by `wrap_text` when a prefixed line (bullet, blockquote,
    /// footnote, or checkbox) is encountered.
    pub(super) fn handle_prefix_line(
        &mut self,
        state: &mut ParagraphState,
        prefix_line: &PrefixLine<'_>,
    ) {
        self.flush_paragraph(state);
        self.append_wrapped_with_prefix(prefix_line);
    }
}