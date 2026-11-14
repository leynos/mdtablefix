//! Heading normalization helpers.
//!
//! This module converts Setext-style headings (underlined with sequences of three or
//! more `=` or `-` characters) into ATX headings that use leading hash markers.
//! Normalising the heading style allows downstream processing such as wrapping to
//! treat the headings consistently.
//!
//! A candidate line is converted only when it is paragraph text, so a line that
//! is itself a block start keeps its underline instead of swallowing it. The
//! private `is_setext_text` predicate applies the same block-start check.

use tracing::trace;

use crate::{
    classify::{
        ClassifyCtx,
        LineClass,
        ListContinuationState,
        classify_line_with_body,
        is_atx_heading_line,
        is_setext_pair,
    },
    wrap::{
        BlockKind,
        FenceObservation,
        FenceTracker,
        LinkReferenceMatcher,
        classify_residual_block,
    },
};

mod prefix;

use prefix::{content_indent_width, has_unmatched_prefix, shared_prefix_len};

/// The parsed input values and context admitted by the production pair detector.
struct ParsedSetextHeading {
    /// ATX level selected from the admitted underline's `=` or `-` marker.
    level: usize,
    /// Byte length of the character-aligned source prefix retained in output.
    prefix_len: usize,
    /// Candidate suffix after the production parser removes surrounding whitespace.
    text: String,
    /// The actual preceding-paragraph and prefix-agreement state used for admission.
    underline_context: ClassifyCtx,
}

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

        let candidate = SetextCandidate::new(
            line,
            lines.get(idx + 1).map(String::as_str),
            link_matcher,
            None,
        );
        if setext_text_lines[idx]
            && let Some(emitted) = candidate.detect_verified_heading()
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
        let continuation_indent = if is_fence_line(fence) {
            lists.reset();
            None
        } else {
            let classified = classify_line_with_body(line, &ClassifyCtx::default());
            lists.observe(line, &classified)
        };

        let next_line = lines.get(idx + 1).map(String::as_str);
        let candidate = SetextCandidate::new(line, next_line, link_matcher, continuation_indent);
        if is_setext_text_line(fence, candidate) {
            setext_text_lines[idx] = true;
            lists.reset();
            idx += 2;
        } else {
            idx += 1;
        }
    }

    setext_text_lines
}

/// Reports whether a line lies on a fence boundary or inside fenced code.
///
/// Both halves are the tracker's answer for this line: a marker opens or closes
/// a fence, and `is_in_fence` covers every line between markers. Either way the
/// line is literal, so list continuation state must not be updated from it.
fn is_fence_line(fence: FenceObservation) -> bool { fence.is_fence_marker || fence.is_in_fence }

/// A candidate Setext heading, with everything its detection needs.
///
/// The text, the line beneath it, the link matcher, and the list continuation
/// indent travel together through every probe below; a heading is only ever
/// decided from all four. Bundling them also keeps the probes from being
/// handed a text line and a matcher that came from different scans.
#[derive(Clone, Copy)]
struct SetextCandidate<'a> {
    /// The prospective heading text.
    line: &'a str,
    /// The line beneath it, which may be the underline.
    underline: Option<&'a str>,
    /// The matcher consulted for reference-link text.
    link_matcher: LinkReferenceMatcher,
    /// Indentation of an enclosing list item, when there is one.
    continuation_indent: Option<usize>,
}

impl<'a> SetextCandidate<'a> {
    /// Pairs a text line with the line beneath it and the surrounding context.
    fn new(
        line: &'a str,
        underline: Option<&'a str>,
        link_matcher: LinkReferenceMatcher,
        continuation_indent: Option<usize>,
    ) -> Self {
        Self {
            line,
            underline,
            link_matcher,
            continuation_indent,
        }
    }

    /// Constructs the replacement heading and independently checks its ATX class.
    ///
    /// For example, `Title` followed by `===` returns `Some("# Title")`.
    /// The sequence kernel proves construction; this guard checks the emitted
    /// line, so a kernel that drifted from the emitted form would still be
    /// caught here rather than producing a heading the classifier rejects.
    fn detect_verified_heading(self) -> Option<String> {
        let underline = self.underline?;
        let heading = detect_setext_heading(
            self.line,
            Some(underline),
            self.link_matcher,
            self.continuation_indent,
        )?;
        let emitted = convert_setext(&heading, self.line, underline)?;
        is_atx_heading_line(&emitted, &ClassifyCtx::default()).then_some(emitted)
    }
}

/// Reports whether `line` is the text half of a valid Setext heading pair.
///
/// Prose outside fences is tested against the same structural predicate as
/// [`convert_setext_headings`]; a fenced line is literal content and can never
/// form a heading.
fn is_setext_text_line(fence: FenceObservation, candidate: SetextCandidate<'_>) -> bool {
    if is_fence_line(fence) {
        return false;
    }
    candidate.detect_verified_heading().is_some()
}

/// Marks both lines of every valid Setext heading: its text and its underline.
///
/// Wrapping consults it, because a `===` underline joined onto its text becomes
/// prose (issue #562), and heading text is left unwrapped as ATX heading text
/// is. `format_breaks` makes the matching decision for a `---` underline
/// through the line classifier.
pub(crate) fn setext_heading_lines(lines: &[String]) -> Vec<bool> {
    let mut heading = vec![false; lines.len()];
    for (index, is_text) in setext_text_lines(lines).into_iter().enumerate() {
        if is_text {
            for slot in heading.iter_mut().skip(index).take(2) {
                *slot = true;
            }
        }
    }
    heading
}

/// Parses a Setext pair into its level, retained prefix, text, and actual underline context.
///
/// The candidate is rejected when the underline prefix differs, the text is a block start, or
/// either line belongs to an indented code block; those checks prevent structural lines from being
/// consumed as heading text.
/// For example, `> Title` followed by `> ===` yields level one, prefix length two,
/// text `Title`, and an agreeing preceding-paragraph context.
fn detect_setext_heading(
    line: &str,
    underline: Option<&str>,
    link_matcher: LinkReferenceMatcher,
    continuation_indent: Option<usize>,
) -> Option<ParsedSetextHeading> {
    let underline = underline?;
    let (prefix_len, underline_context) =
        validated_setext_prefix_context(line, underline, continuation_indent)?;
    if !is_setext_pair(line, &ClassifyCtx::default(), underline, &underline_context) {
        return None;
    }
    let text = line[prefix_len..].trim();
    if text.is_empty() {
        return None;
    }
    if !is_setext_text(text, true, link_matcher) {
        return None;
    }
    if !crate::classify_kernel::setext::valid_setext_prefix(
        &line[..prefix_len].chars().collect::<Vec<_>>(),
    ) {
        trace!(
            prefix_len,
            "refusing a Setext pair with an invalid retained prefix"
        );
        return None;
    }

    let marker = underline[prefix_len..].trim().chars().next()?;
    let level = if marker == '=' { 1 } else { 2 };
    Some(ParsedSetextHeading {
        level,
        prefix_len,
        text: text.to_string(),
        underline_context,
    })
}

/// Validates native prefix and container admission before structural pair matching.
/// For example, `> Title`/`> ===` with no continuation yields prefix length two
/// and the actual agreeing preceding-paragraph context.
fn validated_setext_prefix_context(
    line: &str,
    underline: &str,
    continuation_indent: Option<usize>,
) -> Option<(usize, ClassifyCtx)> {
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
    if !line[..prefix_len]
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

    let underline_context = ClassifyCtx::following(LineClass::ParagraphText, prefixes_agree);
    Some((prefix_len, underline_context))
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
/// Builds an ATX heading while retaining the source indentation or blockquote prefix.
///
/// The structural conversion decision and emitted ATX class are checked
/// through the production classifier. The complete scalar assembly is verified.
/// For example, prefix `>>`, level two, and text `Title` produce `>> ## Title`.
fn convert_setext(
    heading: &ParsedSetextHeading,
    candidate: &str,
    underline: &str,
) -> Option<String> {
    crate::classify_kernel::setext::convert_accepted_setext_seq(
        &candidate.chars().collect::<Vec<_>>(),
        &ClassifyCtx::default(),
        &underline.chars().collect::<Vec<_>>(),
        &heading.underline_context,
        &candidate[..heading.prefix_len].chars().collect::<Vec<_>>(),
        heading.level,
        &heading.text.chars().collect::<Vec<_>>(),
    )
    .map(|heading| heading.into_iter().collect())
}

#[cfg(test)]
#[path = "headings_tests.rs"]
mod tests;
