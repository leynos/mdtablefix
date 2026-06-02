//! Paragraph wrapping utilities shared by `wrap_text`.
//!
//! These helpers keep paragraph logic focused on buffer management while
//! deferring inline wrapping to `inline::wrap_preserving_code`.

use std::borrow::Cow;

use unicode_width::UnicodeWidthStr;

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
}

pub(super) enum ContinuationMode {
    /// Join continuations using normal Markdown soft-break spacing.
    Normalize,
    /// Join without adding a synthetic space after an opener at EOL.
    TightCodeSpan,
    /// Emit the original source lines instead of rewrapping ambiguous input.
    VerbatimFlush,
}
/// Tracks buffered paragraph content and its shared indentation.
pub(super) struct ParagraphState {
    /// Stores buffered paragraph segments and whether each ends with a hard break.
    buf: Vec<(String, bool)>,
    /// Stores the leading indentation reused for wrapped continuation lines.
    indent: String,
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
        let available = self.width.saturating_sub(prefix_width).max(1);
        self.append_wrapped_with_prefix_width(line, available);
    }

    pub(super) fn append_wrapped_with_prefix_width(
        &mut self,
        line: &PrefixLine<'_>,
        available: usize,
    ) {
        let prefix = line.prefix.as_ref();
        let prefix_width = UnicodeWidthStr::width(prefix);
        let indent_str: String = prefix.chars().take_while(|c| c.is_whitespace()).collect();
        let indent_width = UnicodeWidthStr::width(indent_str.as_str());
        let continuation_prefix = if line.repeat_prefix {
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

    pub(super) fn emit_pending_with_verbatim_continuation(
        &mut self,
        pending: PendingPrefix,
        continuation: &str,
        hard_break: bool,
    ) {
        let prefix = pending.prefix;
        let mut first_line = format!("{prefix}{rest}", rest = pending.rest);
        if (pending.hard_break || hard_break) && !first_line.ends_with("  ") {
            first_line.push_str("  ");
        }
        self.out.push(first_line);

        let prefix_width = UnicodeWidthStr::width(prefix.as_str());
        let continuation_prefix = if pending.repeat_prefix {
            prefix
        } else {
            " ".repeat(prefix_width)
        };
        self.out
            .push(format!("{continuation_prefix}{continuation}"));
    }

    /// Flushes the buffered paragraph into wrapped output lines.
    ///
    /// `state` supplies the buffered segments and remembered indent. This
    /// method returns no value, clears the state when flushing completes, and
    /// preserves hard-break segments as distinct wrapped emissions.
    pub(super) fn flush_paragraph(&mut self, state: &mut ParagraphState) {
        if let Some(pending) = state.pending_prefix.take() {
            state.buf.clear();
            state.indent.clear();

            if pending.continuation_mode == ContinuationMode::VerbatimFlush {
                self.out.extend(pending.original_lines);
                return;
            }

            let rest = trim_code_span_edge_spaces(&pending.rest);
            let prefix_line = PrefixLine {
                prefix: Cow::Owned(pending.prefix),
                rest: rest.as_ref(),
                repeat_prefix: pending.repeat_prefix,
            };
            self.append_wrapped_with_prefix_width(&prefix_line, pending.rest_width);
            if pending.hard_break
                && let Some(last) = self.out.last_mut()
                && !last.ends_with("  ")
            {
                last.push_str("  ");
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

        if let Some((fence_len, open_tail)) = parse_open_code_span(prefix_line.rest) {
            let prefix = prefix_line.prefix.as_ref().to_string();
            let prefix_width = UnicodeWidthStr::width(prefix.as_str());
            let opener_at_eol = open_tail.trim().is_empty();
            state.pending_prefix = Some(PendingPrefix {
                prefix,
                rest: prefix_line.rest.to_string(),
                original_lines: vec![format!(
                    "{prefix}{rest}",
                    prefix = prefix_line.prefix.as_ref(),
                    rest = prefix_line.rest,
                )],
                rest_width: self.width.saturating_sub(prefix_width).max(1),
                repeat_prefix: prefix_line.repeat_prefix,
                hard_break: false,
                open_fence_len: Some(fence_len),
                continuation_mode: if opener_at_eol {
                    ContinuationMode::TightCodeSpan
                } else {
                    ContinuationMode::Normalize
                },
            });
            return;
        }

        self.append_wrapped_with_prefix(prefix_line);
    }
}
mod tests {
    use std::borrow::Cow;

    use proptest::prelude::*;
    use unicode_width::UnicodeWidthStr;

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

        let mut quoted_out = Vec::new();
        let mut quoted_writer = ParagraphWriter::new(&mut quoted_out, 10);
        let mut quoted_state = ParagraphState::default();
        quoted_writer.handle_prefix_line(
            &mut quoted_state,
            &PrefixLine {
                prefix: Cow::Borrowed("> "),
                rest: "alpha beta gamma",
                repeat_prefix: true,
            },
        );
        assert_eq!(
            quoted_out,
            vec![
                "> alpha".to_string(),
                "> beta".to_string(),
                "> gamma".to_string(),
            ]
        );
    }

    #[test]
    fn wrap_with_prefix_accounts_for_unicode_wide_prefixes() {
        let mut out = Vec::new();
        let mut writer = ParagraphWriter::new(&mut out, 7);
        writer.wrap_with_prefix("「 ", "  ", "ab cd");
        assert_eq!(out, vec!["「 ab".to_string(), "  cd".to_string()]);
    }

    proptest! {
        #[test]
        fn paragraph_writer_preserves_prefixes_and_width(
            words in proptest::collection::vec("[a-z]{1,6}", 1..=8),
            width in 20usize..=60,
            indent in 0usize..=4,
        ) {
            let prefix = format!("{}- ", " ".repeat(indent));
            let continuation = " ".repeat(UnicodeWidthStr::width(prefix.as_str()));
            let text = words.join(" ");
            let mut out = Vec::new();
            let mut writer = ParagraphWriter::new(&mut out, width);

            writer.wrap_with_prefix(&prefix, &continuation, &text);

            prop_assert!(!out.is_empty());
            prop_assert!(out[0].starts_with(&prefix));
            for line in out.iter().skip(1) {
                prop_assert!(line.starts_with(&continuation));
            }
            for line in &out {
                prop_assert!(
                    UnicodeWidthStr::width(line.as_str()) <= width,
                    "wrapped line exceeded width {width}: {line:?}",
                );
            }
        }
    }
}

mod code_span_trim;
