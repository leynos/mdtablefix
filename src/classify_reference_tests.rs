//! Independent, regex-based reference for the ten-class structural grammar.
//!
//! The fence, bullet, and thematic-break expressions originate in revision
//! `5c3b18e8d0eee078d0b58183c28adadb91e1aeb3`, before `classify_line` existed.
//! This is a declarative reference for the unified contract, not a claim that
//! the historical callers agreed. It deliberately tightens ATX separators,
//! ordered-list length, and Markdown whitespace, and classifies quoted bodies.
//! The reference never calls production scanners, parsers, or predicates;
//! tests separately exercise the production path to compare outcomes.

use regex::Regex;

use super::LineClass;

static INDENT: std::sync::LazyLock<Regex> =
    lazy_regex!(r"^[ \t]*", "reference indentation regex should compile");
static QUOTE: std::sync::LazyLock<Regex> =
    lazy_regex!(r"^([ \t]*)> ?", "reference quote regex should compile");
static FENCE: std::sync::LazyLock<Regex> =
    lazy_regex!(r"^(`{3,}|~{3,})", "reference fence regex should compile");
static ATX: std::sync::LazyLock<Regex> =
    lazy_regex!(r"^#{1,6}(?:[ \t]|$)", "reference ATX regex should compile");
static TABLE: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^\|*[ \t]*:?-+:?[ \t]*(?:\|[ \t]*:?-+:?[ \t]*)*\|?$",
    "reference table delimiter regex should compile",
);
static SETEXT: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(?:={3,}|-{3,})$",
    "reference Setext regex should compile"
);
static BREAK: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(?:(?:[ \t]*\*){3,}|(?:[ \t]*-){3,}|(?:[ \t]*_){3,})[ \t]*$",
    "reference thematic-break regex should compile",
);
static LIST: std::sync::LazyLock<Regex> = lazy_regex!(
    r"^(?:[-*+]|[0-9]{1,9}[.)])[ \t]",
    "reference list regex should compile",
);

/// Independently carried fence, prior classification, and exact prefix state.
#[derive(Default)]
pub(super) struct ReferenceState {
    open: Option<String>,
    previous: Option<LineClass>,
    prefix: String,
}

impl ReferenceState {
    /// Classifies and advances one reference line; `***` starts a thematic break.
    pub(super) fn observe(&mut self, line: &str) -> LineClass {
        let (prefix, body, literal) = reference_parts(line);
        let trimmed = body.trim_matches([' ', '\t']);
        let class = self.classify(trimmed, literal, prefix == self.prefix);
        if class == LineClass::FenceMarker {
            self.open = if self.open.is_some() {
                None
            } else {
                FENCE.find(trimmed).map(|run| run.as_str().to_owned())
            };
        }
        self.previous = Some(class);
        self.prefix = prefix.to_owned();
        class
    }

    /// Resolves precedence; a matching closing run ends an existing fence.
    fn classify(&self, body: &str, literal: bool, prefix_agrees: bool) -> LineClass {
        if let Some(open) = &self.open {
            let closes = !literal
                && body.len() >= open.len()
                && body
                    .bytes()
                    .all(|byte| Some(&byte) == open.as_bytes().first());
            return if closes {
                LineClass::FenceMarker
            } else {
                LineClass::Literal
            };
        }
        if body.is_empty() {
            return LineClass::Blank;
        }
        if literal {
            return LineClass::Literal;
        }
        self.classify_body(body, prefix_agrees)
    }

    /// Applies regex grammar; `|---|` is a delimiter rather than a table row.
    fn classify_body(&self, body: &str, prefix_agrees: bool) -> LineClass {
        if FENCE.is_match(body) {
            LineClass::FenceMarker
        } else if ATX.is_match(body) {
            LineClass::AtxHeading
        } else if body.contains('|') && TABLE.is_match(body) {
            LineClass::TableDelimiter
        } else if body.starts_with('|') {
            LineClass::TableRow
        } else if self.previous == Some(LineClass::ParagraphText)
            && prefix_agrees
            && SETEXT.is_match(body)
        {
            LineClass::SetextUnderline
        } else if BREAK.is_match(body) {
            LineClass::ThematicBreak
        } else if LIST.is_match(body) {
            LineClass::ListItem
        } else {
            LineClass::ParagraphText
        }
    }
}

/// Measures independent indentation; a tab after one space adds three columns.
fn indentation_width(indent: &str, column: usize) -> usize {
    indent.bytes().fold(0, |width, byte| {
        if byte == b'\t' {
            width + 4 - (column + width) % 4
        } else {
            width + 1
        }
    })
}

/// Splits regex quote tokens; `> > title` retains `> > ` as its exact prefix.
fn reference_parts(line: &str) -> (&str, &str, bool) {
    let initial = INDENT.find(line).map_or("", |found| found.as_str());
    let mut column = indentation_width(initial, 0);
    if column >= 4 {
        return ("", line, true);
    }
    let mut offset = initial.len();
    while let Some(captures) = QUOTE.captures(&line[offset..]) {
        let indent = captures.get(1).map_or("", |found| found.as_str());
        let width = indentation_width(indent, column);
        if width >= 4 {
            break;
        }
        let token = captures.get(0).map_or("", |found| found.as_str());
        column = (column + width + token.len() - indent.len()) % 4;
        offset += token.len();
    }
    let body = &line[offset..];
    let indent = INDENT.find(body).map_or("", |found| found.as_str());
    (
        &line[..offset],
        body,
        indentation_width(indent, column) >= 4,
    )
}

/// Historical prefix detection rejected every hash-leading body as a heading.
/// For example, `#word` changes from a heading prefix to paragraph text.
fn historical_heading(body: &str) -> bool { body.trim_start().starts_with('#') }

/// Historical bullet grammar allowed Unicode digits and arbitrary digit counts.
/// For example, `1234567890. text` is now paragraph text.
fn historical_list(body: &str) -> bool {
    static HISTORICAL: std::sync::LazyLock<Regex> = lazy_regex!(
        r"^\s*(?:[-*+]|\d+[.)])\s+",
        "historical list regex should compile",
    );
    HISTORICAL.is_match(body)
}

#[rstest::rstest]
#[case::atx_separator("#word", true, false, LineClass::ParagraphText)]
#[case::atx_level_limit("####### title", true, false, LineClass::ParagraphText)]
#[case::ordered_length("1234567890. text", false, true, LineClass::ParagraphText)]
#[case::unicode_digits("١. text", false, true, LineClass::ParagraphText)]
#[case::unicode_separator("1.\u{a0}text", false, true, LineClass::ParagraphText)]
fn deliberate_historical_grammar_changes(
    #[case] line: &str,
    #[case] was_heading: bool,
    #[case] was_list: bool,
    #[case] expected: LineClass,
) {
    assert_eq!(historical_heading(line), was_heading);
    assert_eq!(historical_list(line), was_list);
    assert_eq!(ReferenceState::default().observe(line), expected);
    assert_eq!(
        super::classify_line(line, &super::ClassifyCtx::default()),
        expected
    );
}

#[rstest::rstest]
#[case::nested_quotes(&["> > title", "> > ---"], &[LineClass::ParagraphText, LineClass::SetextUnderline])]
#[case::prefix_mismatch(&["> title", ">> ---"], &[LineClass::ParagraphText, LineClass::ThematicBreak])]
#[case::fence_lengths(&["````", "```", "````"], &[LineClass::FenceMarker, LineClass::Literal, LineClass::FenceMarker])]
#[case::quoted_indented_fence(
    &[">   ```rust", ">   literal", ">   ```", ">   prose"],
    &[LineClass::FenceMarker, LineClass::Literal, LineClass::FenceMarker, LineClass::ParagraphText],
)]
#[case::indented_literal(&["\t***"], &[LineClass::Literal])]
#[case::four_columns(&["    # title"], &[LineClass::Literal])]
#[case::blank_literal(&["    "], &[LineClass::Blank])]
#[case::table_precedence(&["|---|---|", "---"], &[LineClass::TableDelimiter, LineClass::ThematicBreak])]
fn reference_context_is_independent(#[case] lines: &[&str], #[case] expected: &[LineClass]) {
    let mut reference = ReferenceState::default();
    let actual = lines
        .iter()
        .map(|line| reference.observe(line))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    let mut production = super::ProductionState::default();
    let actual = lines
        .iter()
        .map(|line| production.observe(line))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn thematic_break_tab_prefix_is_literal_under_the_unified_contract() {
    static HISTORICAL_BREAK: std::sync::LazyLock<Regex> = lazy_regex!(
        r"^[ ]{0,3}((?:[ \t]*\*){3,}|(?:[ \t]*-){3,}|(?:[ \t]*_){3,})[ \t]*$",
        "historical thematic-break regex should compile",
    );
    assert!(HISTORICAL_BREAK.is_match("\t***"));
    assert_eq!(
        ReferenceState::default().observe("\t***"),
        LineClass::Literal
    );
    assert!(!crate::classify::is_canonical_break_line(
        "\t***",
        &super::ClassifyCtx::default()
    ));
    let source = vec!["\t***".to_owned()];
    assert_eq!(crate::format_breaks(&source)[0].as_ref(), "\t***");
}

#[rstest::rstest]
#[case::quoted_prose("> text", LineClass::ParagraphText)]
#[case::quoted_heading("> # title", LineClass::AtxHeading)]
#[case::quoted_break("> ***", LineClass::ThematicBreak)]
fn quoted_body_replaces_historical_container_role(#[case] line: &str, #[case] expected: LineClass) {
    static HISTORICAL_QUOTE: std::sync::LazyLock<Regex> = lazy_regex!(
        r"^(\s*(?:>\s*)+)(.*)$",
        "historical quote regex should compile",
    );
    assert!(HISTORICAL_QUOTE.is_match(line));
    assert_eq!(ReferenceState::default().observe(line), expected);
    assert_eq!(
        super::classify_line(line, &super::ClassifyCtx::default()),
        expected
    );
}

#[rstest::rstest]
#[case::non_breaking_space("\u{a0}")]
#[case::vertical_tab("\u{b}")]
fn markdown_blank_is_narrower_than_historical_unicode_trim(#[case] line: &str) {
    assert!(line.trim().is_empty());
    assert_eq!(
        ReferenceState::default().observe(line),
        LineClass::ParagraphText
    );
    assert_eq!(
        super::classify_line(line, &super::ClassifyCtx::default()),
        LineClass::ParagraphText,
    );
}

#[rstest::rstest]
#[case::empty_cells("| |", LineClass::TableRow)]
#[case::spaced_dashes("| - - |", LineClass::TableRow)]
#[case::colons_without_dash("|::|", LineClass::TableRow)]
#[case::nonleading_malformed_cells("::|::", LineClass::ParagraphText)]
fn delimiter_cells_are_stricter_than_the_historical_character_sieve(
    #[case] line: &str,
    #[case] expected: LineClass,
) {
    static HISTORICAL_SEPARATOR: std::sync::LazyLock<Regex> = lazy_regex!(
        r"^[\s|:-]+$",
        "historical table separator regex should compile",
    );
    assert!(line.contains('|') && HISTORICAL_SEPARATOR.is_match(line));
    assert_eq!(ReferenceState::default().observe(line), expected);
    assert_eq!(
        super::classify_line(line, &super::ClassifyCtx::default()),
        expected
    );
}

#[test]
fn paragraph_context_reinterprets_the_historical_thematic_break() {
    assert!(BREAK.is_match("---"));
    let lines = ["2024 revenue", "---"];
    let expected = [LineClass::ParagraphText, LineClass::SetextUnderline];
    let mut reference = ReferenceState::default();
    let mut production = super::ProductionState::default();
    assert_eq!(lines.map(|line| reference.observe(line)), expected);
    assert_eq!(lines.map(|line| production.observe(line)), expected);
}
