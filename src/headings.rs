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

use crate::wrap::{
    BlockKind,
    FenceTracker,
    LinkReferenceMatcher,
    classify_block,
    is_fence,
    leading_indent,
};

/// Convert Setext-style headings into ATX (`#`) headings.
///
/// Lines that are part of fenced code blocks are left unchanged. The function preserves
/// leading blockquote markers and indentation shared by the heading and its underline.
#[must_use]
pub fn convert_setext_headings(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let link_matcher = LinkReferenceMatcher::production();
    let mut fence_tracker = FenceTracker::default();
    let mut idx = 0;

    while idx < lines.len() {
        let line = &lines[idx];

        let fence = fence_tracker.observe_source_line(line);
        if fence.is_fence_marker || fence.is_in_fence {
            out.push(line.clone());
            idx += 1;
            continue;
        }

        if let Some((level, prefix_len, text)) =
            detect_setext_heading(line, lines.get(idx + 1).map(String::as_str), link_matcher)
        {
            let prefix = &line[..prefix_len];
            out.push(build_heading_line(prefix, level, &text));
            idx += 2;
            continue;
        }

        out.push(line.clone());
        idx += 1;
    }

    out
}

fn detect_setext_heading(
    line: &str,
    underline: Option<&str>,
    link_matcher: LinkReferenceMatcher,
) -> Option<(usize, usize, String)> {
    let underline = underline?;
    if line.trim().is_empty() {
        return None;
    }

    let prefix_len = shared_prefix_len(line, underline);
    if has_unmatched_prefix(line, underline) {
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

    let text = line[prefix_len..].trim();
    if text.is_empty() {
        return None;
    }
    if !is_setext_text(text, link_matcher) {
        return None;
    }

    let underline_body = underline[prefix_len..].trim();
    if underline_body.is_empty() {
        return None;
    }

    let marker = underline_body.chars().next()?;
    if marker != '=' && marker != '-' {
        return None;
    }
    if !underline_body.chars().all(|c| c == marker) {
        return None;
    }
    if underline_body.len() < 3 {
        return None;
    }

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
/// text. Block kinds come from [`crate::wrap::classify_block`], so this pass and
/// the wrapper agree on what a block start is.
///
/// A digit-prefixed candidate stays eligible.
/// [`BlockKind::DigitPrefix`] marks a line the wrapper measures specially, not
/// a block; `2024 revenue` is ordinary paragraph text in Markdown.
///
/// HTML blocks are outside the formatter's grammar and are not screened here.
/// The only HTML support the project has is the `<table>` conversion in
/// `crate::html`, which runs before this pass and replaces the lines it
/// recognizes.
fn is_setext_text(text: &str, link_matcher: LinkReferenceMatcher) -> bool {
    if is_fence(text).is_some() {
        trace!(
            payload_len = text.len(),
            "refusing a Setext candidate that is a fence marker"
        );
        return false;
    }
    if is_table_delimiter_row(text) {
        trace!(
            payload_len = text.len(),
            "refusing a Setext candidate that is a table delimiter row"
        );
        return false;
    }

    match classify_block(text, link_matcher) {
        None | Some(BlockKind::DigitPrefix) => true,
        Some(
            kind @ (BlockKind::Heading
            | BlockKind::ThematicBreak
            | BlockKind::Bullet
            | BlockKind::Blockquote
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
    }
}

/// Determine whether a stripped candidate is a table delimiter row.
///
/// A delimiter row is table syntax rather than paragraph text, so the `---`
/// below it is a thematic break and not an underline for it.
/// `| --- | --- |` above `---` was converted into the single line
/// `## | --- | --- |`: the break was consumed, the table above lost its
/// delimiter row, and the orphaned header row was then padded differently on
/// the next pass, so the output never settled.
///
/// The `|` is required. A bare `---` is a thematic break, which
/// [`classify_block`] already refuses, and a break above another break must
/// stay two breaks. Alignment markers and dashes alone are covered by the
/// delimiter row's own pipe, so `|---|---|`, `| --- | --- |`, and `--- | ---`
/// are all refused while a paragraph that merely contains a pipe, such as
/// `Text with > inside | here`, still converts.
///
/// The pattern is the one the table parser already uses to find the delimiter
/// row, so the heading pass and the table pass agree on what one is.
fn is_table_delimiter_row(text: &str) -> bool {
    text.contains('|') && crate::table::SEP_RE.is_match(text)
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

/// Determine whether a line and its underline disagree on indentation or blockquote prefix.
///
/// Setext headings must repeat blockquote (`>`) markers and indentation on both lines. When the
/// prefixes differ we leave the text untouched so blockquote paragraphs or code blocks are not
/// promoted to headings.
fn has_unmatched_prefix(line: &str, underline: &str) -> bool {
    let line_prefix = prefix_of_indent_or_quote(line);
    let underline_prefix = prefix_of_indent_or_quote(underline);
    line_prefix != underline_prefix && (line_prefix > 0 || underline_prefix > 0)
}

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

fn build_heading_line(prefix: &str, level: usize, text: &str) -> String {
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

fn needs_space_after(prefix: &str) -> bool {
    !prefix.is_empty() && !prefix.chars().last().is_some_and(char::is_whitespace)
}

#[cfg(test)]
#[path = "headings_tests.rs"]
mod tests;
