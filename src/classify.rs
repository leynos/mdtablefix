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
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct OpenFence {
    /// Character repeated by the fence marker.
    marker: char,
    /// Number of marker characters in the opening fence.
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
#[derive(Clone, Default)]
pub(crate) struct ClassifyCtx {
    /// Whether the line is within an already-open fenced region.
    is_in_fence: bool,
    /// Opening marker whose compatible closing marker may end that region.
    open_fence: Option<OpenFence>,
    /// Structural class of the immediately preceding source line.
    previous: Option<LineClass>,
    /// Indentation and blockquote prefix of the immediately preceding line.
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
/// Classifies one source line using the shared structural precedence.
#[must_use]
pub fn classify_line(line: &str, ctx: &ClassifyCtx) -> LineClass {
    if line.trim().is_empty() {
        return LineClass::Blank;
    }

    let parts = line_parts(line);
    if parts.is_literal {
        if line.starts_with('\t') && is_thematic_break(line) {
            return LineClass::ThematicBreak;
        }
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

/// Prefix-stripped portions of one line used by structural scanners.
struct LineParts<'a> {
    /// Leading indentation and blockquote prefix retained by conversions.
    prefix: &'a str,
    /// Content after the structural prefix.
    body: &'a str,
    /// Whether indentation makes the content literal code.
    is_literal: bool,
}

/// Splits a line into a blockquote-aware prefix and its structural body.
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

/// Measures indentation columns and the following byte offset from `start`.
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

/// Reports whether `body` begins with a three-or-more marker fence.
fn is_fence_marker(body: &str) -> bool {
    let trimmed = body.trim_start_matches([' ', '\t']);
    let Some(marker) = trimmed.chars().next().filter(|c| matches!(c, '`' | '~')) else {
        return false;
    };
    trimmed.chars().take_while(|c| *c == marker).count() >= 3
}

/// Reports whether `body` is a compatible closing marker for `open`.
fn is_closing_fence(body: &str, open: OpenFence) -> bool {
    let trimmed = body.trim();
    let marker_len = trimmed.chars().take_while(|c| *c == open.marker).count();
    marker_len >= open.marker_len && marker_len == trimmed.len()
}

/// Reports whether `body` starts with a valid ATX marker and separator.
fn is_atx_heading(body: &str) -> bool {
    let trimmed = body.trim_start_matches([' ', '\t']);
    let hash_len = trimmed.chars().take_while(|c| *c == '#').count();
    hash_len > 0
        && trimmed
            .chars()
            .nth(hash_len)
            .is_none_or(char::is_whitespace)
}

/// Reports whether `body` has valid table delimiter cells.
fn is_table_delimiter(body: &str) -> bool {
    let trimmed = body.trim();
    if !trimmed.contains('|') {
        return false;
    }
    let without_leading_pipe = trimmed.trim_start_matches('|');
    let cells = without_leading_pipe
        .strip_suffix('|')
        .unwrap_or(without_leading_pipe);
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
        && without_trailing_colon
            .chars()
            .all(|character| character == '-')
}

/// Reports whether `body` is one uniform three-or-more Setext marker run.
fn is_setext_underline(body: &str) -> bool {
    let trimmed = body.trim();
    let Some(marker) = trimmed.chars().next().filter(|c| matches!(c, '=' | '-')) else {
        return false;
    };
    trimmed.chars().count() >= 3 && trimmed.chars().all(|c| c == marker)
}

/// Reports whether `body` is a three-or-more thematic-break marker run.
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

/// Reports whether `body` begins an ordered or unordered list item.
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

    /// Gives every class a positive witness and a close non-member witness.
    #[rstest]
    #[case(LineClass::ParagraphText, "2024 revenue", "# heading")]
    #[case(LineClass::AtxHeading, "# heading", "#heading")]
    #[case(LineClass::SetextUnderline, "---", "--")]
    #[case(LineClass::TableDelimiter, "|---|---|", "| value |")]
    #[case(LineClass::TableRow, "| value |", "value | value")]
    #[case(LineClass::FenceMarker, "```rust", "``rust")]
    #[case(LineClass::ThematicBreak, "___", "__")]
    #[case(LineClass::ListItem, "- item", "-item")]
    #[case(LineClass::Blank, "   ", "text")]
    #[case(LineClass::Literal, "    code", " code")]
    fn every_class_has_positive_and_negative_examples(
        #[case] expected: LineClass,
        #[case] positive: &str,
        #[case] negative: &str,
    ) {
        let context = if expected == LineClass::SetextUnderline {
            ClassifyCtx::following("candidate", LineClass::ParagraphText)
        } else {
            ClassifyCtx::default()
        };

        assert_eq!(classify_line(positive, &context), expected);
        assert_ne!(classify_line(negative, &context), expected);
    }

    #[test]
    fn uses_preceding_paragraph_and_prefix_for_setext() {
        let context = ClassifyCtx::following("> title", LineClass::ParagraphText);
        assert_eq!(classify_line("> ---", &context), LineClass::SetextUnderline);
        assert_eq!(classify_line("---", &context), LineClass::ThematicBreak);
    }
}
