//! Line buffer utilities for `wrap_preserving_code`.
//!
//! This module encapsulates the mutable state required to accumulate tokens into
//! wrapped lines while reusing allocations between iterations.

use unicode_width::UnicodeWidthStr;

#[derive(Default)]
pub(crate) struct LineBuffer {
    text: String,
    width: usize,
    last_split: Option<usize>,
}

impl LineBuffer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn text(&self) -> &str {
        self.text.as_str()
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn push_token(&mut self, token: &str) {
        if token.len() == 1 && ".?!,:;".contains(token) && self.text.trim_end().ends_with('`') {
            let trimmed_len = self.text.trim_end_matches(char::is_whitespace).len();
            if trimmed_len < self.text.len() {
                let removed = &self.text[trimmed_len..];
                let removed_width = UnicodeWidthStr::width(removed);
                self.text.truncate(trimmed_len);
                self.width = self.width.saturating_sub(removed_width);
                self.last_split = self
                    .text
                    .char_indices()
                    .rev()
                    .find(|(_, ch)| ch.is_whitespace())
                    .map(|(idx, ch)| idx + ch.len_utf8());
            }
        }

        self.text.push_str(token);
        self.width += UnicodeWidthStr::width(token);
        if token.chars().all(char::is_whitespace) {
            self.last_split = Some(self.text.len());
        }
    }

    pub(crate) fn push_span(&mut self, tokens: &[String], start: usize, end: usize) {
        for tok in &tokens[start..end] {
            self.push_token(tok.as_str());
        }
    }

    pub(crate) fn push_non_whitespace_span(&mut self, tokens: &[String], start: usize, end: usize) {
        for tok in &tokens[start..end] {
            if tok.chars().all(char::is_whitespace) {
                continue;
            }
            self.push_token(tok.as_str());
        }

        // No whitespace was appended; keep split unset.
        self.last_split = None;
    }

    pub(crate) fn flush_into(&mut self, lines: &mut Vec<String>) {
        if self.text.is_empty() {
            return;
        }
        lines.push(std::mem::take(&mut self.text));
        self.width = 0;
        self.last_split = None;
    }

    pub(crate) fn split_with_span(
        &mut self,
        lines: &mut Vec<String>,
        tokens: &[String],
        start: usize,
        end: usize,
        width: usize,
    ) -> bool {
        let Some(pos) = self.last_split else {
            return false;
        };

        let (head_bounds, trimmed_tail_start) = {
            let (head, tail) = self.text.split_at(pos);
            let trimmed_head = head.trim_end();
            let trimmed_head_len = trimmed_head.len();
            let trailing_ws = &head[trimmed_head_len..];
            let head_bounds = if trimmed_head_len == 0 {
                None
            } else if trailing_ws.chars().count() > 1 {
                Some((0, pos))
            } else {
                Some((0, trimmed_head_len))
            };

            let trimmed_tail = tail.trim_start();
            let trimmed_tail_start = pos + (tail.len() - trimmed_tail.len());
            (head_bounds, trimmed_tail_start)
        };

        if let Some((start_idx, end_idx)) = head_bounds {
            lines.push(self.text[start_idx..end_idx].to_owned());
        }

        self.text.drain(..trimmed_tail_start);
        for tok in &tokens[start..end] {
            self.text.push_str(tok);
        }

        self.width = UnicodeWidthStr::width(self.text.as_str());
        if end > start && tokens[end - 1].chars().all(char::is_whitespace) && !self.text.is_empty()
        {
            self.last_split = Some(self.text.len());
        } else {
            self.last_split = None;
        }

        if self.width > width {
            lines.push(self.text.trim_end().to_string());
            self.text.clear();
            self.width = 0;
            self.last_split = None;
        }

        true
    }

    pub(crate) fn flush_trailing_whitespace(
        &mut self,
        lines: &mut Vec<String>,
        tokens: &[String],
        start: usize,
        end: usize,
    ) -> bool {
        if end != tokens.len() {
            return false;
        }
        if !tokens[start..end]
            .iter()
            .all(|tok| tok.chars().all(char::is_whitespace))
        {
            return false;
        }

        if self.text.is_empty() {
            self.last_split = None;
            return true;
        }

        for tok in &tokens[start..end] {
            self.text.push_str(tok);
        }
        lines.push(std::mem::take(&mut self.text));
        self.width = 0;
        self.last_split = None;
        true
    }
}
