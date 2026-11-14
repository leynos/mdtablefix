//! Fence-line parsing shared by the tracker and the batch classifier.
//!
//! This module owns the one regex that turns a source line into fence
//! components, the blockquote-prefix view it needs, and the reduction of that
//! parse into the kernel's [`LineFeatures`]. It is separate from the parent so
//! the tracker's state machine and its tracing stay readable on their own, and
//! so both the streaming [`FenceTracker`](super::FenceTracker) and the batch
//! [`classify_regions`](super::classify_regions) path reach the same parse
//! rather than keeping a second notion of what a fence line is.

use regex::Regex;

use super::LineFeatures;
use crate::wrap::BlockquotePrefix;

/// Match indentation, a three-or-more marker run, and its complete info text.
///
/// The info string is retained verbatim because only a closing fence with
/// spaces or tabs after its marker may terminate an existing fenced block.
pub(super) static FENCE_RE: std::sync::LazyLock<Regex> =
    // Capture: indent, fence run of 3+ backticks/tilde, and the full info string (incl. leading
    // spaces)
    lazy_regex!(
        r"^(\s*)(`{3,}|~{3,})([^\r\n]*)$",
        "wrapping fence delimiter and info string pattern should compile",
    );

/// Return fence components if the line starts a fenced code block.
///
/// The function captures:
/// - the leading indentation,
/// - the fence marker itself (three or more backticks or tildes),
/// - the full trailing “info string” (including any leading spaces and attributes).
///
/// # Examples
///
/// ```rust
/// use mdtablefix::wrap::is_fence;
/// assert_eq!(is_fence("```rust"), Some(("", "```", "rust")));
/// assert_eq!(is_fence("``` rust"), Some(("", "```", " rust")));
/// assert_eq!(is_fence("``` rust linenums"), Some(("", "```", " rust linenums")));
/// assert_eq!(is_fence("> > ```rust"), Some(("> > ", "```", "rust")));
/// assert!(is_fence("not a fence").is_none());
/// ```
#[doc(hidden)]
#[must_use]
#[rustfmt::skip]
pub fn is_fence(line: &str) -> Option<(&str, &str, &str)> {
    let context = FenceLine::parse(line);
    is_inner_fence(context.inner).map(|(inner_indent, fence, info)| {
        // `inner` is always a suffix of `line`, so the difference of their
        // lengths is the prefix's byte length. Deriving it keeps the two views
        // from drifting apart, which a stored copy could not guarantee.
        let prefix_len = line.len() - context.inner.len();
        let indent = &line[..prefix_len + inner_indent.len()];
        (indent, fence, info)
    })
}

/// Match a fence after any blockquote prefix has been removed.
///
/// Returning borrowed capture slices keeps the original info string available
/// for the closing-fence whitespace check.
///
/// Exposed to the child `kernel` module, which classifies whole documents by
/// reusing the parent's single regex parse rather than a second one.
#[rustfmt::skip]
pub(super) fn is_inner_fence(line: &str) -> Option<(&str, &str, &str)> {
    FENCE_RE.captures(line).map(|cap| {
        let inner_indent = cap.get(1).map_or("", |m| m.as_str());
        let fence  = cap.get(2).map_or("", |m| m.as_str());
        let info   = cap.get(3).map_or("", |m| m.as_str());
        (inner_indent, fence, info)
    })
}

/// Reduce a parsed fence line to the kernel's line features.
///
/// The `None` result means the line is not fence-shaped, which still has to be
/// stepped because it may drop below the opening blockquote depth and end the
/// fence implicitly.
///
/// `FENCE_RE` guarantees a non-empty marker run, so the first character always
/// exists; the `?` would only fire if the regex disagreed with the kernel's
/// notion of a fence marker.
fn line_features(depth: usize, parsed: Option<(&str, &str, &str)>) -> Option<LineFeatures> {
    let (_indent, fence, info) = parsed?;
    let mut chars = fence.chars();
    let marker = chars.next()?;
    let marker_len = chars.count() + 1;
    // CommonMark forbids an info string on a closing fence: a same-marker
    // line carrying trailing text is literal content, not a close. Only ASCII
    // spaces and tabs may follow a closing marker, so avoid the Unicode-aware
    // `trim`, which would wrongly accept a no-break space (U+00A0) or form
    // feed (U+000C) as blank trailing whitespace.
    let trailing_blank = info.bytes().all(|b| b == b' ' || b == b'\t');
    Some(LineFeatures::fence(
        depth,
        marker,
        marker_len,
        trailing_blank,
    ))
}

/// Reduce one complete source line, blockquote prefix included, to kernel
/// features.
///
/// The child `kernel` module reuses this so a whole-document classification
/// runs the same parse as the streaming [`FenceTracker`], rather than keeping
/// a second notion of what a fence line is.
pub(super) fn features_of(line: &str) -> LineFeatures {
    let context = FenceLine::parse(line);
    line_features(context.depth, is_inner_fence(context.inner))
        .unwrap_or_else(|| LineFeatures::prose(context.depth))
}

/// Reduce an already prefix-stripped line to kernel features at `depth`.
///
/// `FenceTracker::observe_inner` needs the same reduction [`features_of`]
/// performs, but its caller has already removed the blockquote prefix, so the
/// depth is supplied directly instead of parsed again.
pub(super) fn features_at_depth(depth: usize, inner: &str) -> Option<LineFeatures> {
    line_features(depth, is_inner_fence(inner))
}

/// The prefix-stripped view needed while classifying a source fence line.
pub(super) struct FenceLine<'a> {
    /// Content after the optional blockquote prefix.
    pub(super) inner: &'a str,
    /// Blockquote nesting depth at which this line appears.
    pub(super) depth: usize,
}

impl<'a> FenceLine<'a> {
    /// Parse the optional blockquote prefix.
    pub(super) fn parse(line: &'a str) -> Self {
        BlockquotePrefix::parse(line).map_or(
            Self {
                inner: line,
                depth: 0,
            },
            |prefix| Self {
                inner: prefix.inner(),
                depth: prefix.depth(),
            },
        )
    }
}
