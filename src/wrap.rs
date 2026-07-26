//! Utilities for wrapping Markdown lines.
//!
//! These helpers reflow paragraphs and list items while preserving inline code
//! spans, fenced code blocks, and other prefixes. Width calculations rely on
//! `UnicodeWidthStr::width` from the `unicode-width` crate as described in
//! `docs/architecture.md#unicode-width-handling`.
//!
//! The [`Token`] enum and [`tokenize_markdown`] function are public so callers
//! can perform custom token-based processing.

use std::borrow::Cow;

use tracing::trace;

mod block;
mod blockquote;
mod continuation;
mod fence;
mod inline;
mod link_reference;
mod observer;
mod paragraph;
mod pending;
mod tokenize;
mod tracing_adapter;
#[cfg(test)]
pub(crate) mod tracing_snapshot_support;
use block::{BULLET_RE, FOOTNOTE_RE};
pub(crate) use block::{BlockKind, classify_block, leading_indent};
pub use blockquote::BlockquotePrefix;
use continuation::apply_continuation_chunk;
pub(crate) use fence::{FenceObservation, ObservedFence};
/// Fence-detection utilities re-exported for downstream callers.
///
/// [`FenceTracker`] maintains fenced code-block state across lines, which is
/// useful for callers that process Markdown incrementally. [`is_fence`]
/// inspects one line and returns the fence components (indentation, marker,
/// info string) when the line opens a fenced code block, or `None` otherwise.
pub use fence::{FenceTracker, is_fence};
pub(crate) use link_reference::{LinkReferenceMatcher, LinkTitleWindow, LinkTitleWindowOutcome};
use paragraph::{ParagraphState, ParagraphWriter, PrefixLine};
use pending::handle_pending_continuation;
/// Token emitted by the `tokenize::segment_inline` parser and used by
/// higher-level wrappers.
///
/// Downstream callers inspect [`Token<'a>`] when implementing bespoke
/// wrapping logic. The `'a` lifetime parameter ties each token to the source
/// text, avoiding unnecessary allocation.
///
/// Re-export these so callers of [`crate::textproc`] can implement custom
/// transformations without depending on internal modules.
pub use tokenize::Token;
#[doc(inline)]
pub use tokenize::tokenize_markdown;
// Re-exported for unit tests; not used in production code.
#[cfg(test)]
pub(crate) use tokenize::{continuation_begins_with_closing_fence, has_unclosed_code_span};
pub(crate) use tokenize::{has_odd_backslash_escape_bytes, link_or_image_span};

// Permit GFM task list markers with flexible spacing and missing post-marker
// spaces in Markdown.

/// Return whether a line is an indented code block with visible content.
///
/// Blank indented lines remain paragraph separators; only four-column
/// indentation followed by a non-whitespace character is protected here.
fn is_indented_code_line(line: &str) -> bool {
    let (indent_width, first_content_byte) = leading_indent(line);
    indent_width >= 4
        && line[first_content_byte..]
            .chars()
            .any(|c| !c.is_whitespace())
}

/// Return whether a line belongs to a table or a table-separator boundary.
///
/// These lines are emitted verbatim because reflowing their pipes or separator
/// dashes would change the table grammar before the table formatter sees it.
fn is_table_or_separator(line: &str) -> bool {
    line.trim_start().starts_with('|') || crate::table::SEP_RE.is_match(line.trim())
}

/// Returns whether `line` must be emitted verbatim rather than wrapped.
///
/// Thematic breaks are included even though [`is_table_or_separator`] already
/// passes `---` through: that accidental match relies on the table-separator
/// pattern, which rejects `***`, `___`, `- - -`, and the underscore run
/// emitted by `--breaks`. Recognising the break directly keeps all of those on
/// their own line, so a second `--wrap` pass cannot absorb a normalised break
/// into the surrounding paragraph.
fn is_passthrough_block(block_kind: Option<BlockKind>, line: &str) -> bool {
    is_table_or_separator(line)
        || matches!(
            block_kind,
            Some(
                BlockKind::Heading
                    | BlockKind::MarkdownlintDirective
                    | BlockKind::LinkReferenceDefinition
                    | BlockKind::ThematicBreak,
            )
        )
        || line.trim().is_empty()
        || is_indented_code_line(line)
}

/// Parse a list or footnote prefix, retaining any outer blockquote prefix.
///
/// The returned `PrefixLine` marks whether a prefix must repeat on subsequent
/// lines and borrows all source slices so verbatim syntax can be reconstructed.
fn prefix_line<'a>(
    inner_content: &'a str,
    blockquote: Option<BlockquotePrefix<'a>>,
) -> Option<PrefixLine<'a>> {
    let outer_prefix = blockquote.map(|prefix| prefix.raw_prefix());

    if let Some(cap) = BULLET_RE.captures(inner_content) {
        let inner_prefix = cap.get(1).map(|m| m.as_str())?;
        let rest = cap.get(2).map(|m| m.as_str())?;
        return Some(PrefixLine {
            prefix: outer_prefix.map_or_else(
                || Cow::Borrowed(inner_prefix),
                |outer| Cow::Owned(format!("{outer}{inner_prefix}")),
            ),
            rest,
            repeat_prefix: false,
            outer_prefix: outer_prefix.map(Cow::Borrowed),
        });
    }

    if let Some(cap) = FOOTNOTE_RE.captures(inner_content) {
        let prefix = cap.get(1).map(|m| m.as_str())?;
        let marker = cap.get(2).map(|m| m.as_str())?;
        let rest = cap.get(3).map(|m| m.as_str())?;
        let inner_prefix = format!("{prefix}{marker}");
        return Some(PrefixLine {
            prefix: Cow::Owned(format!(
                "{}{inner_prefix}",
                outer_prefix.unwrap_or_default()
            )),
            rest,
            repeat_prefix: false,
            outer_prefix: outer_prefix.map(Cow::Borrowed),
        });
    }

    let Some(blockquote) = blockquote else {
        trace!(
            line_len = inner_content.len(),
            "prefix_line found no supported prefix"
        );
        return None;
    };
    Some(PrefixLine {
        prefix: Cow::Borrowed(blockquote.raw_prefix()),
        rest: inner_content,
        repeat_prefix: true,
        outer_prefix: Some(Cow::Borrowed(blockquote.raw_prefix())),
    })
}

/// Split a source line into its original spelling, inner content, and block
/// context before any paragraph state is changed.
///
/// Keeping both views borrowed from the input lets the dispatcher preserve
/// verbatim prefixes while sending only inner content to wrapping logic.
#[derive(Clone, Copy)]
struct LineContext<'a> {
    /// The complete source line, retained for verbatim output.
    original: &'a str,
    /// The content after any blockquote prefix.
    inner: &'a str,
    /// The parsed blockquote prefix, if this line has one.
    blockquote: Option<BlockquotePrefix<'a>>,
    /// The recognised block-level construct, if any.
    block_kind: Option<BlockKind>,
}

/// The line data needed before dispatching a line into fence and link state.
#[derive(Clone, Copy)]
struct PreambleLine<'a> {
    /// The complete source line for verbatim emission.
    original: &'a str,
    /// The content after a blockquote prefix.
    inner: &'a str,
    /// The active blockquote nesting depth used by fence tracking.
    depth: usize,
}

/// Remove Markdown hard-break markers while retaining whether the break was
/// explicit.
///
/// Trailing spaces, an HTML break, or an odd trailing backslash all represent
/// a hard break. The returned text is safe for paragraph accumulation because
/// only the marker is removed; authored content remains otherwise unchanged.
fn line_break_parts(line: &str) -> (String, bool) {
    let trimmed_end = line.trim_end();
    let text_without_html_breaks = trimmed_end
        .trim_end_matches("<br>")
        .trim_end_matches("<br/>")
        .trim_end_matches("<br />");

    let is_trailing_spaces = line.ends_with("  ");
    let is_html_br = trimmed_end != text_without_html_breaks;
    let backslash_count = trimmed_end.chars().rev().take_while(|&c| c == '\\').count();
    let is_backslash_escape = backslash_count % 2 == 1;
    let hard_break = is_trailing_spaces || is_html_br || is_backslash_escape;
    let text = text_without_html_breaks
        .trim_start()
        .trim_end_matches(' ')
        .to_string();
    (text, hard_break)
}

/// Collapse whitespace-only passthrough lines to the canonical empty line.
///
/// Verbatim constructs keep their source spelling, except that a whitespace
/// only separator is normalised so repeated formatting does not accumulate
/// insignificant indentation.
fn normalized_passthrough_line(line: &str) -> &str {
    if !line.is_empty() && line.trim().is_empty() {
        trace!(
            line_len = line.len(),
            "normalizing whitespace-only passthrough line"
        );
        ""
    } else {
        line
    }
}

/// Consume a continuation whose blockquote prefix still matches pending state.
///
/// This fast path avoids reparsing the line as a new block and therefore keeps
/// lazy blockquote continuations in the same paragraph.
fn try_blockquote_fast_path(
    line: LineContext<'_>,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
) -> bool {
    let outer_matches_pending = line.blockquote.is_some_and(|prefix| {
        state
            .pending_prefix
            .as_ref()
            .is_some_and(|pending| pending.outer_prefix.as_deref() == Some(prefix.raw_prefix()))
    });
    if !outer_matches_pending || line.block_kind.is_some() {
        return false;
    }

    let (text, hard_break) = line_break_parts(line.inner);
    apply_continuation_chunk(&text, line.original, hard_break, writer, state);
    true
}

/// Emit a line that must remain structurally intact and stop paragraph parsing.
///
/// Link-reference definitions are observed before emission so a following
/// standalone title can be classified without altering the definition itself.
fn try_passthrough_block(
    line: LineContext<'_>,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
    link_matcher: LinkReferenceMatcher,
    link_title_window: &mut link_reference::LinkTitleWindow,
) -> bool {
    if !is_passthrough_block(line.block_kind, line.inner) {
        return false;
    }

    trace!(
        ?line.block_kind,
        line_len = line.original.len(),
        "passing a block boundary through unchanged"
    );

    if matches!(line.block_kind, Some(BlockKind::LinkReferenceDefinition)) {
        link_title_window.observe_definition(line.inner, link_matcher);
    }
    let emitted = normalized_passthrough_line(line.original);
    writer.push_verbatim(state, emitted);
    true
}

/// Handle fence and link-title context that must be known before paragraph
/// dispatch.
///
/// Returning `true` means the line was emitted or consumed by that preamble
/// state, so the caller must not also feed it to paragraph wrapping.
fn handle_line_preamble(
    line: PreambleLine<'_>,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
    fence_tracker: &mut FenceTracker,
    link_matcher: LinkReferenceMatcher,
    link_title_window: &mut link_reference::LinkTitleWindow,
) -> bool {
    if fence::handle_fence_line(
        line.original,
        line.inner,
        line.depth,
        writer,
        state,
        fence_tracker,
    ) {
        link_title_window.observe_fence_context();
        return true;
    }

    if fence_tracker.in_fence(line.depth) {
        link_title_window.observe_fence_context();
        writer.push_verbatim(state, line.original);
        return true;
    }

    if let Some(outcome) = link_title_window.observe_next_line(line.inner, link_matcher)
        && outcome == link_reference::LinkTitleWindowOutcome::EmitVerbatim
    {
        writer.push_verbatim(state, line.original);
        return true;
    }

    false
}

/// Route a line through pending prefixes, verbatim block boundaries, or a new
/// list/blockquote prefix.
///
/// The boolean result records ownership of the line: a handled line has
/// already updated writer state and must not be appended as ordinary prose.
fn dispatch_continuation(
    line: LineContext<'_>,
    writer: &mut ParagraphWriter<'_>,
    state: &mut ParagraphState,
    link_matcher: LinkReferenceMatcher,
    link_title_window: &mut link_reference::LinkTitleWindow,
) -> bool {
    if state.pending_prefix.is_some()
        && handle_pending_continuation(line, writer, state, link_matcher, link_title_window)
    {
        return true;
    }

    if try_passthrough_block(line, writer, state, link_matcher, link_title_window) {
        return true;
    }

    if let Some(prefix_line) = prefix_line(line.inner, line.blockquote) {
        writer.handle_prefix_line(state, &prefix_line);
        return true;
    }

    false
}

/// Wrap text lines to the given width.
#[must_use]
pub fn wrap_text(lines: &[String], width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut state = ParagraphState::default();
    let mut writer = ParagraphWriter::new(&mut out, width);
    // Track fenced code blocks so wrapping honours shared fence semantics.
    let mut fence_tracker = FenceTracker::default();
    let link_matcher = link_reference::LinkReferenceMatcher::production();
    let mut link_title_window = link_reference::LinkTitleWindow::default();

    for line in lines {
        let blockquote = BlockquotePrefix::parse(line);
        let current_depth = blockquote.map_or(0, |prefix| prefix.depth());
        let inner_content = blockquote.map_or(line.as_str(), |prefix| prefix.inner());

        if handle_line_preamble(
            PreambleLine {
                original: line,
                inner: inner_content,
                depth: current_depth,
            },
            &mut writer,
            &mut state,
            &mut fence_tracker,
            link_matcher,
            &mut link_title_window,
        ) {
            continue;
        }

        let block_kind = classify_block(inner_content, link_matcher);
        if dispatch_continuation(
            LineContext {
                original: line,
                inner: inner_content,
                blockquote,
                block_kind,
            },
            &mut writer,
            &mut state,
            link_matcher,
            &mut link_title_window,
        ) {
            continue;
        }

        state.note_indent(inner_content);
        let (text, hard_break) = line_break_parts(inner_content);
        state.push(text, hard_break);
    }

    writer.flush_paragraph(&mut state);
    out
}

#[cfg(test)]
mod tests;

#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub mod bench_internals;
