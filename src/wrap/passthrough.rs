//! Predicates for lines the paragraph wrapper emits verbatim.
//!
//! Tables, thematic breaks, headings, directives, link reference definitions,
//! blank lines and indented code keep their source spelling: reflowing any of
//! them would change the block the line belongs to. Kept apart from `wrap.rs`
//! so the dispatch there stays readable and under the file-size limit.

use tracing::trace;

use super::{BlockKind, leading_indent};

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
pub(super) fn is_passthrough_block(block_kind: Option<BlockKind>, line: &str) -> bool {
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

/// Collapse whitespace-only passthrough lines to the canonical empty line.
///
/// Verbatim constructs keep their source spelling, except that a whitespace
/// only separator is normalised so repeated formatting does not accumulate
/// insignificant indentation.
pub(super) fn normalized_passthrough_line(line: &str) -> &str {
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
