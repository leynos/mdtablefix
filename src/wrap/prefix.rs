//! Parsing of the list, footnote and blockquote prefixes a wrapped line keeps.
//!
//! The prefix is split from the text so the paragraph writer can repeat or
//! indent it on continuation lines while wrapping only the text after it.

use std::borrow::Cow;

use tracing::trace;

use super::{
    BlockquotePrefix,
    PrefixLine,
    block::{BULLET_RE, FOOTNOTE_RE},
};

/// Parse a list or footnote prefix, retaining any outer blockquote prefix.
///
/// The returned `PrefixLine` marks whether a prefix must repeat on subsequent
/// lines and borrows all source slices so verbatim syntax can be reconstructed.
pub(super) fn prefix_line<'a>(
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
