//! State carried while a prefixed inline-code span crosses source lines.
//!
//! The types and prefix helpers here are owned by paragraph continuation
//! handling. Callers outside `wrap` must use `wrap_text` rather than composing
//! this internal state directly.

use unicode_width::UnicodeWidthStr;

/// Buffers a prefixed line whose inline code span continues on later source lines.
pub(in crate::wrap) struct PendingPrefix {
    /// Stores the bullet, blockquote, or footnote prefix.
    pub(in crate::wrap) prefix: String,
    /// Stores joined content after the prefix.
    pub(in crate::wrap) rest: String,
    /// Stores source lines for ambiguous verbatim fallbacks.
    pub(in crate::wrap) original_lines: Vec<String>,
    /// Records byte offsets of spaces inserted across source lines.
    pub(in crate::wrap) synthetic_join_spaces: Vec<usize>,
    /// Stores the display width available after the prefix.
    pub(in crate::wrap) rest_width: usize,
    /// Marks blockquotes whose full prefix repeats on continuations.
    pub(in crate::wrap) repeat_prefix: bool,
    /// Stores the blockquote portion repeated before an indented inner prefix.
    pub(in crate::wrap) outer_prefix: Option<String>,
    /// Marks a closing continuation with a Markdown hard break.
    pub(in crate::wrap) hard_break: bool,
    /// Stores the active inline-code fence length.
    pub(in crate::wrap) open_fence_len: Option<usize>,
    /// Controls normalization versus verbatim fallback.
    pub(in crate::wrap) continuation_mode: ContinuationMode,
    /// Marks whether the original prefix has already been emitted.
    pub(in crate::wrap) used_prefix: bool,
    /// Controls whether later continuation prose can join this segment.
    pub(in crate::wrap) tail_reflow: TailReflow,
}

/// Controls how a pending prefixed continuation should be joined or emitted.
#[derive(Debug, PartialEq)]
pub(in crate::wrap) enum ContinuationMode {
    /// Join continuations using normal Markdown soft-break spacing.
    Normalize,
    /// Join without adding a synthetic space after an opener at EOL.
    TightCodeSpan,
    /// Emit the original source lines instead of rewrapping ambiguous input.
    VerbatimFlush,
}

/// Records whether a resolved span may absorb later continuation prose.
#[derive(Debug, PartialEq)]
pub(in crate::wrap) enum TailReflow {
    /// Continue buffering prose until the prefixed block ends.
    Allowed,
    /// Flush after span resolution because the input closed and reopened spans.
    Disallowed,
}

/// Selects the original or continuation prefix for the next emitted segment.
pub(in crate::wrap) fn pending_prefix_for_next_segment(pending: &mut PendingPrefix) -> String {
    if pending.used_prefix {
        continuation_prefix_for(
            pending.prefix.as_str(),
            pending.repeat_prefix,
            pending.outer_prefix.as_deref(),
        )
    } else {
        pending.used_prefix = true;
        pending.prefix.clone()
    }
}

/// Builds the visual continuation prefix for a parsed Markdown prefix.
pub(in crate::wrap) fn continuation_prefix_for(
    prefix: &str,
    repeat_prefix: bool,
    outer_prefix: Option<&str>,
) -> String {
    if repeat_prefix {
        return prefix.to_string();
    }

    let prefix_width = UnicodeWidthStr::width(prefix);
    if let Some(outer_prefix) = outer_prefix {
        let outer_width = UnicodeWidthStr::width(outer_prefix);
        return format!(
            "{outer_prefix}{}",
            " ".repeat(prefix_width.saturating_sub(outer_width))
        );
    }
    let indent_str: String = prefix.chars().take_while(|c| c.is_whitespace()).collect();
    let indent_width = UnicodeWidthStr::width(indent_str.as_str());
    format!("{}{}", indent_str, " ".repeat(prefix_width - indent_width))
}
