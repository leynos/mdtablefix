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
    /// Stores buffered paragraph segments and whether each ends with a hard break.
    buf: Vec<(String, bool)>,
    /// Stores the leading indentation reused for wrapped continuation lines.
    indent: String,
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
    }

    /// Records the paragraph indent from `line` when the buffer is still empty.
    ///
    /// The `line` parameter is the original input line whose leading
    /// whitespace may become the continuation prefix. This method returns no
    /// value, updates `indent` only for the first buffered segment, and never
    /// panics.
    pub(super) fn note_indent(&mut self, line: &str) {
        if self.buf.is_empty() {
            self.indent = line.chars().take_while(|c| c.is_whitespace()).collect();
        }
    }

    /// Appends one paragraph segment and its hard-break marker.
    ///
    /// `text` is stored verbatim and `hard_break` records whether the source
    /// line ended with Markdown hard-break spacing. This method returns no
    /// value and keeps buffered segments in input order without panicking.
    pub(super) fn push(&mut self, text: String, hard_break: bool) {
        self.buf.push((text, hard_break));
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
    ///
    /// `state` supplies the buffered segments and remembered indent. This
    /// method returns no value, clears the state when flushing completes, and
    /// preserves hard-break segments as distinct wrapped emissions.
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
        self.append_wrapped_with_prefix(prefix_line);
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::{ParagraphState, ParagraphWriter, PrefixLine};

    #[test]
    fn wrap_with_prefix_emits_single_line_when_text_fits() {
        let mut out = Vec::new();
        let mut writer = ParagraphWriter::new(&mut out, 80);
        writer.wrap_with_prefix("> ", "> ", "hello world");
        assert_eq!(out, vec!["> hello world".to_string()]);
    }

    #[test]
    fn wrap_with_prefix_uses_continuation_prefix_on_wrapped_lines() {
        let mut out = Vec::new();
        let mut writer = ParagraphWriter::new(&mut out, 14);
        writer.wrap_with_prefix("> ", "  ", "alpha beta gamma");
        assert_eq!(out, vec!["> alpha beta".to_string(), "  gamma".to_string()]);
    }

    #[test]
    fn handle_prefix_line_can_repeat_or_change_the_continuation_prefix() {
        let mut out = Vec::new();
        let mut writer = ParagraphWriter::new(&mut out, 14);
        let mut state = ParagraphState::default();
        writer.handle_prefix_line(
            &mut state,
            &PrefixLine {
                prefix: Cow::Borrowed("- [ ] "),
                rest: "alpha beta",
                repeat_prefix: false,
            },
        );
        assert_eq!(
            out,
            vec!["- [ ] alpha".to_string(), "      beta".to_string()]
        );
    }

    #[test]
    fn wrap_with_prefix_accounts_for_unicode_wide_prefixes() {
        let mut out = Vec::new();
        let mut writer = ParagraphWriter::new(&mut out, 7);
        writer.wrap_with_prefix("「 ", "  ", "ab cd");
        assert_eq!(out, vec!["「 ab".to_string(), "  cd".to_string()]);
    }
}
