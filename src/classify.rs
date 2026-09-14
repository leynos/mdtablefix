//! Shared structural classification for one Markdown source line.
//!
//! The classifier is deliberately a small scanner rather than a collection of
//! regular expressions.  Its precedence is blank/literal/fence/ATX/table/
//! Setext/thematic/list/paragraph, with a Setext underline taking precedence
//! over a thematic break only when the supplied previous line is paragraph
//! text with the same indentation and blockquote prefix.  Digit-prefixed text
//! is ordinary [`LineClass::ParagraphText`]: `2024 revenue` remains eligible
//! for Setext conversion.  A leading tab before an otherwise valid thematic
//! break keeps the formatter's historic break-normalisation compatibility.

use tracing::trace;

/// The structural role of a line after its indentation and blockquote prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LineClass {
    /// Text that can form a paragraph and may be the candidate of a Setext pair.
    ParagraphText,
    /// An ATX heading with a hash run followed by whitespace or end of line.
    AtxHeading,
    /// A `===` or `---` underline whose preceding line is compatible prose.
    SetextUnderline,
    /// A table alignment row containing pipes, colons, dashes, and whitespace.
    TableDelimiter,
    /// A pipe-leading table row that is not an alignment row.
    TableRow,
    /// An opening or closing backtick/tilde fence marker.
    FenceMarker,
    /// A three-or-more marker thematic break.
    ThematicBreak,
    /// An unordered or ordered list item marker followed by whitespace.
    ListItem,
    /// A whitespace-only line.
    Blank,
    /// Source content that must remain literal, such as indented or fenced code.
    Literal,
}

/// Fence state needed to distinguish literal fenced contents from markers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OpenFence {
    marker: char,
    marker_len: usize,
}

impl OpenFence {
    /// Records the marker that opened the current fenced region.
    #[must_use]
    pub(crate) const fn new(marker: char, marker_len: usize) -> Self { Self { marker, marker_len } }
}

/// Context that makes a line classification independent of its caller.
///
/// The preceding class and exact indentation/blockquote prefix distinguish a
/// Setext underline from a thematic break.  A fence context marks ordinary
/// fenced contents as [`LineClass::Literal`], while still recognising a valid
/// matching closing marker.
#[derive(Clone, Debug, Default)]
pub(crate) struct ClassifyCtx {
    is_in_fence: bool,
    open_fence: Option<OpenFence>,
    previous: Option<LineClass>,
    previous_prefix: Option<String>,
}

impl ClassifyCtx {
    /// Builds a context for a fenced region.
    #[must_use]
    pub(crate) fn in_fence(open_fence: OpenFence) -> Self {
        Self {
            is_in_fence: true,
            open_fence: Some(open_fence),
            previous: None,
            previous_prefix: None,
        }
    }

    /// Builds the context used to classify the line immediately after `line`.
    #[must_use]
    pub(crate) fn following(line: &str, previous: LineClass) -> Self {
        Self {
            is_in_fence: false,
            open_fence: None,
            previous: Some(previous),
            previous_prefix: Some(line_parts(line).prefix.to_owned()),
        }
    }
}

struct LineParts<'a> {
    prefix: &'a str,
    body: &'a str,
    is_literal: bool,
}

/// Classifies one source line using the shared structural precedence.
#[must_use]
pub(crate) fn classify_line(line: &str, ctx: &ClassifyCtx) -> LineClass {
    if line.trim().is_empty() {
        return LineClass::Blank;
    }

    let parts = line_parts(line);
    if parts.is_literal {
        if line.starts_with('\t') && is_thematic_break(line) {
            return LineClass::ThematicBreak;
        }
        trace!(
            line_len = line.len(),
            "classifying indented source as literal"
        );
        return LineClass::Literal;
    }
    if ctx.is_in_fence {
        return if ctx
            .open_fence
            .is_some_and(|open| is_closing_fence(parts.body, open))
        {
            LineClass::FenceMarker
        } else {
            LineClass::Literal
        };
    }
    if is_fence_marker(parts.body) {
        return LineClass::FenceMarker;
    }
    if is_atx_heading(parts.body) {
        return LineClass::AtxHeading;
    }
    if is_table_delimiter(parts.body) {
        return LineClass::TableDelimiter;
    }
    if parts.body.trim_start().starts_with('|') {
        return LineClass::TableRow;
    }
    if ctx.previous == Some(LineClass::ParagraphText)
        && ctx.previous_prefix.as_deref() == Some(parts.prefix)
        && is_setext_underline(parts.body)
    {
        return LineClass::SetextUnderline;
    }
    if is_thematic_break(parts.body) {
        return LineClass::ThematicBreak;
    }
    if is_list_item(parts.body) {
        return LineClass::ListItem;
    }
    LineClass::ParagraphText
}

fn line_parts(line: &str) -> LineParts<'_> {
    let (outer_width, mut cursor) = indentation_at(line, 0);
    if outer_width >= 4 {
        return LineParts {
            prefix: "",
            body: line,
            is_literal: true,
        };
    }

    loop {
        let quote_start = cursor;
        let (_, after_indent) = indentation_at(line, cursor);
        let Some(after_marker) = line
            .get(after_indent..)
            .and_then(|rest| rest.strip_prefix('>'))
        else {
            break;
        };
        let marker_end = line.len() - after_marker.len();
        cursor = marker_end + usize::from(after_marker.starts_with(' '));
        if quote_start == cursor {
            break;
        }
    }

    let (content_indent, _) = indentation_at(line, cursor);
    LineParts {
        prefix: &line[..cursor],
        body: &line[cursor..],
        is_literal: content_indent >= 4,
    }
}

fn indentation_at(line: &str, start: usize) -> (usize, usize) {
    let mut width = 0;
    let mut cursor = start;
    for byte in line.as_bytes().iter().skip(start) {
        match byte {
            b' ' => {
                width += 1;
                cursor += 1;
            }
            b'\t' => {
                width += 4;
                cursor += 1;
            }
            _ => break,
        }
    }
    (width, cursor)
}

fn is_fence_marker(body: &str) -> bool {
    let trimmed = body.trim_start_matches([' ', '\t']);
    let Some(marker) = trimmed.chars().next().filter(|c| matches!(c, '`' | '~')) else {
        return false;
    };
    trimmed.chars().take_while(|c| *c == marker).count() >= 3
}

fn is_closing_fence(body: &str, open: OpenFence) -> bool {
    let trimmed = body.trim();
    let marker_len = trimmed.chars().take_while(|c| *c == open.marker).count();
    marker_len >= open.marker_len && marker_len == trimmed.len()
}

fn is_atx_heading(body: &str) -> bool {
    let trimmed = body.trim_start_matches([' ', '\t']);
    let hash_len = trimmed.chars().take_while(|c| *c == '#').count();
    hash_len > 0
        && trimmed
            .chars()
            .nth(hash_len)
            .is_none_or(char::is_whitespace)
}

fn is_table_delimiter(body: &str) -> bool {
    let trimmed = body.trim();
    let Some(without_trailing_pipe) = trimmed.strip_suffix('|') else {
        return false;
    };
    let cells = without_trailing_pipe.trim_start_matches('|');
    !cells.is_empty() && cells.split('|').all(is_table_delimiter_cell)
}

/// Reports whether one trimmed table cell is an alignment-marker cell.
///
/// The table pass requires a dash in every cell.  Mirroring that grammar here
/// keeps malformed header rows, including empty cells and `- -`, from being
/// classified as table delimiters before they can reach table reflow.
fn is_table_delimiter_cell(cell: &str) -> bool {
    let trimmed = cell.trim();
    let without_leading_colon = trimmed.strip_prefix(':').unwrap_or(trimmed);
    let without_trailing_colon = without_leading_colon
        .strip_suffix(':')
        .unwrap_or(without_leading_colon);
    !without_trailing_colon.is_empty()
        && without_trailing_colon.chars().all(|character| character == '-')
}

fn is_setext_underline(body: &str) -> bool {
    let trimmed = body.trim();
    let Some(marker) = trimmed.chars().next().filter(|c| matches!(c, '=' | '-')) else {
        return false;
    };
    trimmed.chars().count() >= 3 && trimmed.chars().all(|c| c == marker)
}

fn is_thematic_break(body: &str) -> bool {
    let trimmed = body.trim();
    let Some(marker) = trimmed
        .chars()
        .next()
        .filter(|c| matches!(c, '*' | '-' | '_'))
    else {
        return false;
    };
    let markers = trimmed.chars().filter(|c| *c == marker).count();
    markers >= 3
        && trimmed
            .chars()
            .all(|c| c == marker || matches!(c, ' ' | '\t'))
}

fn is_list_item(body: &str) -> bool {
    let trimmed = body.trim_start_matches([' ', '\t']);
    let mut chars = trimmed.chars();
    match chars.next() {
        Some('-' | '*' | '+') => chars.next().is_some_and(char::is_whitespace),
        Some(first) if first.is_ascii_digit() => {
            let mut marker_end = first.len_utf8();
            for character in chars {
                marker_end += character.len_utf8();
                if matches!(character, '.' | ')') {
                    return trimmed
                        .get(marker_end..)
                        .and_then(|rest| rest.chars().next())
                        .is_some_and(char::is_whitespace);
                }
                if !character.is_ascii_digit() {
                    return false;
                }
            }
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for shared line classification.

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("2024 revenue", LineClass::ParagraphText)]
    #[case("# heading", LineClass::AtxHeading)]
    #[case("#123", LineClass::ParagraphText)]
    #[case("|---|---|", LineClass::TableDelimiter)]
    #[case("|  |  |", LineClass::TableRow)]
    #[case("| - - |", LineClass::TableRow)]
    #[case("| value |", LineClass::TableRow)]
    #[case("```rust", LineClass::FenceMarker)]
    #[case("___", LineClass::ThematicBreak)]
    #[case("\t_\t_\t_\t", LineClass::ThematicBreak)]
    #[case("- item", LineClass::ListItem)]
    #[case("   ", LineClass::Blank)]
    #[case("    code", LineClass::Literal)]
    fn classifies_structural_lines(#[case] line: &str, #[case] expected: LineClass) {
        assert_eq!(classify_line(line, &ClassifyCtx::default()), expected);
    }

    #[test]
    fn uses_preceding_paragraph_and_prefix_for_setext() {
        let context = ClassifyCtx::following("> title", LineClass::ParagraphText);
        assert_eq!(classify_line("> ---", &context), LineClass::SetextUnderline);
        assert_eq!(classify_line("---", &context), LineClass::ThematicBreak);
    }
}
