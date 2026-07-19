//! Semantic parsing for blockquote prefixes used by the wrapping pipeline.
//!
//! [`BlockquotePrefix`] borrows the original line so callers can inspect the
//! quote depth and inner content without allocating or reconstructing text.

use tracing::{debug, trace};

use super::block::BLOCKQUOTE_RE;

/// A parsed Markdown blockquote prefix and its inner content.
///
/// The parser preserves the source spelling of the prefix, including spaces
/// and tabs, while exposing nesting depth independently from that spelling.
///
/// # Examples
///
/// ```
/// use mdtablefix::wrap::BlockquotePrefix;
///
/// let prefix = BlockquotePrefix::parse("> > quoted text")
///     .expect("the line should contain a blockquote prefix");
/// assert_eq!(prefix.raw_prefix(), "> > ");
/// assert_eq!(prefix.depth(), 2);
/// assert_eq!(prefix.inner(), "quoted text");
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockquotePrefix<'a> {
    raw_prefix: &'a str,
    depth: usize,
    inner: &'a str,
}

impl<'a> BlockquotePrefix<'a> {
    /// Parse the leading blockquote markers from `line`.
    #[must_use]
    pub fn parse(line: &'a str) -> Option<Self> {
        let line_len = line.len();
        let Some(captures) = BLOCKQUOTE_RE.captures(line) else {
            trace!(
                line_len,
                reason = "no_blockquote_prefix",
                "blockquote prefix rejected"
            );
            return None;
        };
        let raw_prefix = captures.get(1)?.as_str();
        let inner = captures.get(2)?.as_str();
        let depth = raw_prefix.bytes().filter(|byte| *byte == b'>').count();

        debug!(
            line_len,
            prefix_len = raw_prefix.len(),
            depth,
            inner_len = inner.len(),
            "blockquote prefix parsed"
        );

        Some(Self {
            raw_prefix,
            depth,
            inner,
        })
    }

    /// Return the prefix exactly as written in the source line.
    #[must_use]
    pub fn raw_prefix(&self) -> &'a str { self.raw_prefix }

    /// Return the number of blockquote markers in the prefix.
    #[must_use]
    pub fn depth(&self) -> usize { self.depth }

    /// Return the content following the complete blockquote prefix.
    #[must_use]
    pub fn inner(&self) -> &'a str { self.inner }
}
