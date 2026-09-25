//! Heading normalization helpers.
//!
//! This module converts Setext-style headings (underlined with sequences of three or
//! more `=` or `-` characters) into ATX headings that use leading hash markers.
//! Normalising the heading style allows downstream processing such as wrapping to
//! treat the headings consistently.
//!
//! A candidate line is converted only when it is paragraph text, so a line that
//! is itself a block start keeps its underline instead of swallowing it. See
//! [`is_setext_text`].

use tracing::trace;

use crate::{
    classify::{
        ClassifyCtx,
        LineClass,
        ListContinuationState,
        classify_line_with_body,
        is_atx_heading_line,
        is_setext_text_line,
        is_setext_underline_line,
        quote_depth,
    },
    wrap::{
        BlockKind,
        FenceTracker,
        LinkReferenceMatcher,
        classify_residual_block,
        leading_indent,
    },
};

/// Convert Setext-style headings into ATX (`#`) headings.
///
/// Lines that are part of fenced code blocks are left unchanged. The function preserves
/// leading blockquote markers and indentation shared by the heading and its underline.
#[must_use]
pub fn convert_setext_headings(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let setext_text_lines = setext_text_lines(lines);
    let link_matcher = LinkReferenceMatcher::production();
    let mut idx = 0;

    while idx < lines.len() {
        let line = &lines[idx];

        if setext_text_lines[idx]
            && let Some(emitted) = detect_verified_setext_heading(
                line,
                lines.get(idx + 1).map(String::as_str),
                link_matcher,
                None,
            )
        {
            out.push(emitted);
            idx += 2;
            continue;
        }

        out.push(line.clone());
        idx += 1;
    }

    out
}

/// Marks source lines that are the text half of valid Setext heading pairs.
///
/// The marker respects fenced-code state and uses the same structural predicate
/// as [`convert_setext_headings`]. Callers that must inspect prose before
/// heading conversion can therefore preserve Setext text without duplicating
/// the heading grammar.
#[must_use]
pub(crate) fn setext_text_lines(lines: &[String]) -> Vec<bool> {
    let mut setext_text_lines = vec![false; lines.len()];
    let link_matcher = LinkReferenceMatcher::production();
    let mut fence_tracker = FenceTracker::default();
    let mut lists = ListContinuationState::default();
    let mut idx = 0;

    while idx < lines.len() {
        let line = &lines[idx];
        let fence = fence_tracker.observe_source_line(line);
        let continuation_indent = if fence.is_fence_marker || fence.is_in_fence {
            lists.reset();
            None
        } else {
            let classified = classify_line_with_body(line, &ClassifyCtx::default());
            lists.observe(line, &classified)
        };

        if !fence.is_fence_marker
            && !fence.is_in_fence
            && detect_verified_setext_heading(
                line,
                lines.get(idx + 1).map(String::as_str),
                link_matcher,
                continuation_indent,
            )
            .is_some()
        {
            setext_text_lines[idx] = true;
            lists.reset();
            idx += 2;
        } else {
            idx += 1;
        }
    }

    setext_text_lines
}

/// Builds a Setext replacement only when the verified classifier sees ATX output.
fn detect_verified_setext_heading(
    line: &str,
    underline: Option<&str>,
    link_matcher: LinkReferenceMatcher,
    continuation_indent: Option<usize>,
) -> Option<String> {
    let (level, prefix_len, text) =
        detect_setext_heading(line, underline, link_matcher, continuation_indent)?;
    let emitted = convert_setext(&line[..prefix_len], level, &text);
    is_atx_heading_line(&emitted, &ClassifyCtx::default()).then_some(emitted)
}

/// Parses a Setext heading pair and returns its level, shared prefix length, and text.
///
/// The candidate is rejected when the underline prefix differs, the text is a block start, or
/// either line belongs to an indented code block; those checks prevent structural lines from being
/// consumed as heading text.
fn detect_setext_heading(
    line: &str,
    underline: Option<&str>,
    link_matcher: LinkReferenceMatcher,
    continuation_indent: Option<usize>,
) -> Option<(usize, usize, String)> {
    let underline = underline?;
    if line.trim().is_empty() {
        return None;
    }

    let prefix_len = shared_prefix_len(line, underline);
    let prefixes_agree = !has_unmatched_prefix(line, underline);
    if !prefixes_agree {
        return None;
    }
    if continuation_indent.is_some_and(|indent| content_indent_width(underline) < indent) {
        return None;
    }
    if prefix_len > 0
        && !line[..prefix_len]
            .chars()
            .all(|c| c.is_whitespace() || c == '>')
    {
        return None;
    }
    // Four columns of indentation make the pair an indented code block, where
    // the second line is code rather than an underline. The width is measured
    // on the whole line, before `prefix_len` is removed: the shared prefix
    // swallows the very columns that mark the code block.
    let indent_width = content_indent_width(line);
    if indent_width >= 4 {
        trace!(
            indent_width,
            "refusing a Setext candidate indented as an indented code block"
        );
        return None;
    }

    let candidate_is_paragraph = is_setext_text_line(line, &ClassifyCtx::default());
    let text = line[prefix_len..].trim();
    if text.is_empty() {
        return None;
    }
    if !is_setext_text(text, candidate_is_paragraph, link_matcher) {
        return None;
    }

    if !is_setext_underline_line(
        underline,
        &ClassifyCtx::following(LineClass::ParagraphText, prefixes_agree),
    ) {
        return None;
    }

    let marker = underline[prefix_len..].trim().chars().next()?;
    let level = if marker == '=' { 1 } else { 2 };
    Some((level, prefix_len, text.to_string()))
}

/// Determine whether a stripped candidate is paragraph text.
///
/// Only paragraph text may carry a Setext underline: converting a candidate
/// that is itself a block start swallows the block below it. `## aa` above
/// `---` became the single line `## ## aa`, and the thematic break was lost.
///
/// The candidate is measured after [`shared_prefix_len`] has removed the
/// indentation or blockquote prefix shared with the underline, so a valid
/// quoted heading such as `> Title` above `> -----` still reads as paragraph
/// text. The shared classifier decides structural roles; the residual block
/// matcher checks only forms the classifier does not represent.
///
/// A digit-prefixed candidate stays eligible.
/// [`BlockKind::DigitPrefix`] marks a line the wrapper measures specially, not
/// a block; `2024 revenue` is ordinary paragraph text in Markdown.
///
/// HTML blocks are outside the formatter's grammar and are not screened here.
/// The only HTML support the project has is the `<table>` conversion in
/// `crate::html`, which runs before this pass and replaces the lines it
/// recognizes.
fn is_setext_text(
    text: &str,
    candidate_is_paragraph: bool,
    link_matcher: LinkReferenceMatcher,
) -> bool {
    if !candidate_is_paragraph {
        trace!(
            candidate_is_paragraph,
            payload_len = text.len(),
            "refusing a Setext candidate with a structural line class"
        );
        return false;
    }

    match classify_residual_block(text, link_matcher) {
        None => true,
        Some(
            kind @ (BlockKind::Blockquote
            | BlockKind::FootnoteDefinition
            | BlockKind::LinkReferenceDefinition
            | BlockKind::MarkdownlintDirective),
        ) => {
            trace!(
                ?kind,
                "refusing a Setext candidate that is itself a block start"
            );
            false
        }
        Some(_) => false,
    }
}
/// Returns the indentation width of a line's content, in columns.
///
/// Blockquote markers are consumed before the width is measured, so
/// `>     code` counts as four columns inside the quote. A marker may itself be
/// reached through one to three leading spaces, and those spaces belong to the
/// marker rather than to the content: `   >     code` is four columns too,
/// which is what makes it an indented code block. Tabs count as four columns,
/// matching [`crate::wrap::leading_indent`].
fn content_indent_width(line: &str) -> usize {
    let mut rest = line;

    while let Some(tail) = strip_blockquote_marker(rest) {
        rest = tail;
    }

    leading_indent(rest).0
}

/// Strips one blockquote marker, returning the content that follows it.
///
/// A marker is up to three leading spaces, a `>`, and the single space that may
/// follow it. The wrapper's blockquote prefix accepts the same shape, so both
/// passes measure the quoted content from the same point. Four or more leading
/// spaces are an indented code block rather than a marker, and are left in
/// place for the caller to measure.
fn strip_blockquote_marker(line: &str) -> Option<&str> {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count().min(3);
    let marker = line.get(indent..)?.strip_prefix('>')?;
    Some(marker.strip_prefix(' ').unwrap_or(marker))
}

/// Returns the byte length of the character-aligned prefix shared by a heading and its underline.
fn shared_prefix_len(a: &str, b: &str) -> usize {
    let mut end = 0;
    let mut iter_a = a.char_indices();
    let mut iter_b = b.char_indices();

    loop {
        match (iter_a.next(), iter_b.next()) {
            (Some((idx_a, ch_a)), Some((_, ch_b))) if ch_a == ch_b => {
                end = idx_a + ch_a.len_utf8();
            }
            _ => break,
        }
    }

    end
}

/// Determine whether a line and its underline disagree on blockquote depth.
///
/// Setext headings must remain within one quote level. Up to three spaces of
/// indentation may differ between the text and underline; the classifier
/// independently rejects indented code.
fn has_unmatched_prefix(line: &str, underline: &str) -> bool {
    let line_prefix = prefix_of_indent_or_quote(line);
    let underline_prefix = prefix_of_indent_or_quote(underline);
    let line_depth = quote_depth(&line[..line_prefix]);
    let underline_depth = quote_depth(&underline[..underline_prefix]);
    line_depth != underline_depth
}

/// Returns the byte length of leading indentation and blockquote markers.
fn prefix_of_indent_or_quote(text: &str) -> usize {
    let mut last = 0;
    for (idx, ch) in text.char_indices() {
        if ch.is_whitespace() || ch == '>' {
            last = idx + ch.len_utf8();
            continue;
        }
        break;
    }
    last
}

/// Builds an ATX heading while retaining the source indentation or blockquote prefix.
///
/// The structural conversion decision and emitted ATX class are checked
/// through the production classifier. String assembly stays at this boundary.
fn convert_setext(prefix: &str, level: usize, text: &str) -> String {
    let mut heading = String::new();
    heading.push_str(prefix);
    if needs_space_after(prefix) {
        heading.push(' ');
    }
    heading.push_str(&"#".repeat(level));
    if !text.is_empty() {
        heading.push(' ');
        heading.push_str(text);
    }
    heading
}

/// Reports whether an ATX marker needs a separator after a non-whitespace prefix.
fn needs_space_after(prefix: &str) -> bool {
    !prefix.is_empty() && !prefix.chars().last().is_some_and(char::is_whitespace)
}

#[cfg(test)]
#[path = "headings_tests.rs"]
mod tests;
