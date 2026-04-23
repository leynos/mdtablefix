//! Paragraph wrapping utilities shared by `wrap_text`.
//!
//! These helpers keep paragraph logic focused on buffer management while
//! deferring inline wrapping to `inline::wrap_preserving_code`.

use std::borrow::Cow;

use unicode_width::UnicodeWidthStr;

use super::inline::wrap_preserving_code;

/// Carries the parsed prefix metadata for a line that should be wrapped.
pub(super) struct PrefixLine<'a> {
    /// Stores the literal prefix emitted on the first wrapped line.
    pub(super) prefix: Cow<'a, str>,
    /// Stores the text that follows the prefix and should be wrapped.
    pub(super) rest: &'a str,
    /// Marks whether continuation lines should repeat the full prefix.
    pub(super) repeat_prefix: bool,
}

#[derive(Default)]
/// Tracks buffered paragraph content and its shared indentation.
pub(super) struct ParagraphState {
    buf: Vec<(String, bool)>,
    indent: String,
}

impl ParagraphState {
    /// Clears the buffered paragraph text and remembered indentation.
    pub(super) fn clear(&mut self) {
        self.buf.clear();
        self.indent.clear();
    }

    /// Records the first-line indentation so continuations align visually.
    pub(super) fn note_indent(&mut self, line: &str) {
        if self.buf.is_empty() {
            self.indent = line.chars().take_while(|c| c.is_whitespace()).collect();
        }
    }

    /// Appends a paragraph segment together with its hard-break marker.
    pub(super) fn push(&mut self, text: String, hard_break: bool) {
        self.buf.push((text, hard_break));
    }
}

/// Emits wrapped paragraph lines into the caller-provided output buffer.
pub(super) struct ParagraphWriter<'a> {
    out: &'a mut Vec<String>,
    width: usize,
}

impl<'a> ParagraphWriter<'a> {
    /// Creates a paragraph writer for the given output buffer and width.
    pub(super) fn new(out: &'a mut Vec<String>, width: usize) -> Self { Self { out, width } }

    /// Wraps text with the given first-line and continuation prefixes.
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

    /// Wraps a parsed prefixed line using the correct continuation indentation.
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

    /// Flushes the buffered paragraph into wrapped output lines.
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

    /// Wraps one buffered segment using its shared indentation prefix.
    fn push_wrapped_segment(&mut self, indent: &str, segment: &str) {
        self.wrap_with_prefix(indent, indent, segment);
    }

    /// Flushes any active paragraph and then emits the verbatim line.
    pub(super) fn push_verbatim(&mut self, state: &mut ParagraphState, line: &str) {
        self.flush_paragraph(state);
        self.out.push(line.to_string());
    }

    /// Flushes any active paragraph and wraps the new prefixed line.
    pub(super) fn handle_prefix_line(
        &mut self,
        state: &mut ParagraphState,
        prefix_line: &PrefixLine<'_>,
    ) {
        self.flush_paragraph(state);
        self.append_wrapped_with_prefix(prefix_line);
    }
}
