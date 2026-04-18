//! Paragraph wrapping utilities shared by `wrap_text`.
//!
//! These helpers keep paragraph logic focused on buffer management while
//! deferring inline wrapping to `inline::wrap_preserving_code`.

use std::borrow::Cow;

use unicode_width::UnicodeWidthStr;

use super::inline::wrap_preserving_code;

pub(super) struct PrefixLine<'a> {
    pub(super) prefix: Cow<'a, str>,
    pub(super) rest: &'a str,
    pub(super) repeat_prefix: bool,
}

#[derive(Default)]
pub(super) struct ParagraphState {
    buf: Vec<(String, bool)>,
    indent: String,
}

impl ParagraphState {
    pub(super) fn clear(&mut self) {
        self.buf.clear();
        self.indent.clear();
    }

    pub(super) fn note_indent(&mut self, line: &str) {
        if self.buf.is_empty() {
            self.indent = line.chars().take_while(|c| c.is_whitespace()).collect();
        }
    }

    pub(super) fn push(&mut self, text: String, hard_break: bool) {
        self.buf.push((text, hard_break));
    }
}

pub(super) struct ParagraphWriter<'a> {
    out: &'a mut Vec<String>,
    width: usize,
}

impl<'a> ParagraphWriter<'a> {
    pub(super) fn new(out: &'a mut Vec<String>, width: usize) -> Self { Self { out, width } }

    fn append_wrapped_with_prefix(&mut self, line: &PrefixLine<'_>) {
        let prefix = line.prefix.as_ref();
        let prefix_width = UnicodeWidthStr::width(prefix);
        let available = self.width.saturating_sub(prefix_width).max(1);
        let indent_str: String = prefix.chars().take_while(|c| c.is_whitespace()).collect();
        let indent_width = UnicodeWidthStr::width(indent_str.as_str());
        let wrapped_indent = if line.repeat_prefix {
            prefix.to_string()
        } else {
            format!("{}{}", indent_str, " ".repeat(prefix_width - indent_width))
        };

        let lines = wrap_preserving_code(line.rest, available);
        if lines.is_empty() {
            self.out.push(prefix.to_string());
            return;
        }

        for (index, wrapped_line) in lines.iter().enumerate() {
            if index == 0 {
                self.out.push(format!("{prefix}{wrapped_line}"));
            } else {
                self.out.push(format!("{wrapped_indent}{wrapped_line}"));
            }
        }
    }

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

    fn push_wrapped_segment(&mut self, indent: &str, segment: &str) {
        for line in wrap_preserving_code(segment, self.width - indent.len()) {
            self.out.push(format!("{indent}{line}"));
        }
    }

    pub(super) fn push_verbatim(&mut self, state: &mut ParagraphState, line: &str) {
        self.flush_paragraph(state);
        self.out.push(line.to_string());
    }

    pub(super) fn handle_prefix_line(
        &mut self,
        state: &mut ParagraphState,
        prefix_line: &PrefixLine<'_>,
    ) {
        self.flush_paragraph(state);
        self.append_wrapped_with_prefix(prefix_line);
    }
}
