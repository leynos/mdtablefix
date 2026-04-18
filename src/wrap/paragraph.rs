//! Paragraph wrapping utilities shared by `wrap_text`.
//!
//! These helpers keep paragraph logic focused on buffer management while
//! deferring inline wrapping to `inline::wrap_preserving_code`.

use unicode_width::UnicodeWidthStr;

use super::inline::wrap_preserving_code;

pub(super) struct PrefixLine<'a> {
    pub(super) prefix: String,
    pub(super) rest: &'a str,
    pub(super) repeat_prefix: bool,
}

pub(super) struct ParagraphWriter<'a> {
    out: &'a mut Vec<String>,
    width: usize,
    buf: Vec<(String, bool)>,
    indent: String,
}

impl<'a> ParagraphWriter<'a> {
    pub(super) fn new(out: &'a mut Vec<String>, width: usize) -> Self {
        Self {
            out,
            width,
            buf: Vec::new(),
            indent: String::new(),
        }
    }

    pub(super) fn note_indent(&mut self, line: &str) {
        if self.buf.is_empty() {
            self.indent = line.chars().take_while(|c| c.is_whitespace()).collect();
        }
    }

    pub(super) fn push_wrapped(&mut self, text: String, hard_break: bool) {
        self.buf.push((text, hard_break));
    }

    fn push_wrapped_segment(&mut self, indent: &str, segment: &str) {
        let indent_width = UnicodeWidthStr::width(indent);
        let available = self.width.saturating_sub(indent_width).max(1);
        for line in wrap_preserving_code(segment, available) {
            self.out.push(format!("{indent}{line}"));
        }
    }

    pub(super) fn flush_paragraph(&mut self) {
        if self.buf.is_empty() {
            return;
        }

        let indent = std::mem::take(&mut self.indent);
        let buf = std::mem::take(&mut self.buf);
        let mut segment = String::new();
        for (text, hard_break) in &buf {
            if !segment.is_empty() {
                segment.push(' ');
            }
            segment.push_str(text);
            if *hard_break {
                self.push_wrapped_segment(&indent, &segment);
                segment.clear();
            }
        }

        if !segment.is_empty() {
            self.push_wrapped_segment(&indent, &segment);
        }
    }

    pub(super) fn push_verbatim(&mut self, line: &str) {
        self.flush_paragraph();
        self.out.push(line.to_string());
    }

    pub(super) fn push_blank_line(&mut self) {
        self.flush_paragraph();
        self.out.push(String::new());
    }

    pub(super) fn handle_prefix_line(&mut self, prefix_line: &PrefixLine<'_>) {
        self.flush_paragraph();

        let prefix = prefix_line.prefix.as_str();
        let prefix_width = UnicodeWidthStr::width(prefix);
        let available = self.width.saturating_sub(prefix_width).max(1);
        let indent_str: String = prefix.chars().take_while(|c| c.is_whitespace()).collect();
        let indent_width = UnicodeWidthStr::width(indent_str.as_str());
        let wrapped_indent = if prefix_line.repeat_prefix {
            prefix.to_string()
        } else {
            format!("{}{}", indent_str, " ".repeat(prefix_width - indent_width))
        };

        let lines = wrap_preserving_code(prefix_line.rest, available);
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
}
