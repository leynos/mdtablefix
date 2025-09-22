//! Footnote normalisation utilities.
//!
//! Converts bare numeric references in text to GitHub-flavoured Markdown
//! footnote links and, when eligible, rewrites the trailing numeric list
//! into a footnote block. Eligibility requires that the final contiguous
//! list immediately follows an H2 heading and that no existing footnote
//! definitions (`[^n]:`) appear earlier in the document.

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::LazyLock;

use regex::{Captures, Regex};

static INLINE_FN_RE: LazyLock<Regex> = lazy_regex!(
    r"(?P<pre>^|[^0-9])(?P<punc>[.!?);:])(?P<style>[*_]*)(?P<num>\d+)(?P<boundary>\s|$)",
    "inline footnote reference pattern should compile",
);

static COLON_FN_RE: LazyLock<Regex> = lazy_regex!(
    r"(?P<pre>^|[^0-9])\s+(?P<style>[*_]*)(?P<num>\d+)\s*:(?P<colons>:*)(?P<boundary>\s|[[:punct:]]|$)",
    "space-colon footnote reference pattern should compile",
);

static FOOTNOTE_LINE_RE: LazyLock<Regex> = lazy_regex!(
    r"^(?P<indent>\s*)(?P<num>\d+)[.:]\s+(?P<rest>.*)$",
    "footnote line pattern should compile",
);

use crate::textproc::{Token, push_original_token, tokenize_markdown};
static ATX_HEADING_RE: LazyLock<Regex> = lazy_regex!(
    r"(?x)
        ^\s*
        (?:>+\s*)*
        (?:[-*+]\s+|\d+[.)]\s+)*
        \#{1,6}
        (?:\s|$)
    ",
    "atx heading prefix",
);

/// Extract the components of an inline footnote reference.
#[inline]
fn capture_parts<'a>(caps: &'a Captures<'a>) -> (&'a str, &'a str, &'a str, &'a str, &'a str) {
    (
        &caps["pre"],
        &caps["punc"],
        &caps["style"],
        &caps["num"],
        &caps["boundary"],
    )
}

/// Construct a footnote link from the captured components.
#[inline]
fn build_footnote(pre: &str, punc: &str, style: &str, num: &str, boundary: &str) -> String {
    format!("{pre}{punc}{style}[^{num}]{boundary}")
}

fn convert_inline(text: &str) -> String {
    let out = INLINE_FN_RE.replace_all(text, |caps: &Captures| {
        let (pre, punc, style, num, boundary) = capture_parts(caps);
        build_footnote(pre, punc, style, num, boundary)
    });
    COLON_FN_RE
        .replace_all(&out, |caps: &Captures| {
            let pre = &caps["pre"];
            let style = &caps["style"];
            let num = &caps["num"];
            let colons = &caps["colons"];
            let boundary = &caps["boundary"];
            format!("{pre}{style}[^{num}]:{colons}{boundary}")
        })
        .into_owned()
}

/// Find the trailing block of lines that satisfy a predicate.
///
/// The slice is scanned from the end and trailing blank lines are ignored.
/// The returned `(start, end)` indices delimit the contiguous region of lines
/// whose trimmed contents cause `predicate` to return `true`. Use
/// `lines[start..end]` for slicing.
///
/// # Examples
///
/// ```ignore
/// let lines = vec![
///     "A".to_string(),
///     "1. note".to_string(),
///     "2. more".to_string(),
/// ];
/// let (start, end) = trimmed_range(&lines, |l| l.starts_with('1') || l.starts_with('2'));
/// assert_eq!((start, end), (1, 3));
/// ```
fn trimmed_range<F>(lines: &[String], predicate: F) -> (usize, usize)
where
    F: Fn(&str) -> bool,
{
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |i| i + 1);
    let start = (0..end)
        .rfind(|&i| !predicate(lines[i].trim_end()))
        .map_or(0, |i| i + 1);
    (start, end)
}

/// Identify the trailing block of blank or footnote-like lines.
///
/// Returns `Some((start, end))` when the final block contains at least one
/// footnote line; otherwise `None`.
///
/// # Examples
///
/// ```ignore
/// let lines = vec![
///     "Text".to_string(),
///     " 1. Note".to_string(),
/// ];
/// assert_eq!(footnote_block_range(&lines), Some((1, 2)));
/// ```
fn footnote_block_range(lines: &[String]) -> Option<(usize, usize)> {
    let (start, end) = trimmed_range(lines, |l| {
        l.trim().is_empty() || FOOTNOTE_LINE_RE.is_match(l)
    });
    if start < end
        && lines[start..end]
            .iter()
            .any(|l| FOOTNOTE_LINE_RE.is_match(l))
    {
        Some((start, end))
    } else {
        None
    }
}

/// Determine whether a second-level heading precedes the block.
///
/// # Examples
///
/// ```ignore
/// let lines = vec!["## Footnotes".to_string(), " 1. Note".to_string()];
/// assert!(has_h2_heading_before(&lines, 1));
/// ```
fn has_h2_heading_before(lines: &[String], start: usize) -> bool {
    lines[..start]
        .iter()
        .rfind(|l| !l.trim().is_empty())
        .is_some_and(|l| l.trim_start().starts_with("## "))
}

/// Check for existing footnote definitions before the block.
///
/// Lines that start with an inline reference (e.g., `[^1] note`) are ignored;
/// only definitions like `[^1]: note` cause skipping. Definitions inside fenced code blocks are ignored.
///
/// # Examples
///
/// ```ignore
/// let lines = vec!["[^1]: Old".to_string(), " 2. New".to_string()];
/// assert!(has_existing_footnote_block(&lines, 1));
/// ```
fn has_existing_footnote_block(lines: &[String], start: usize) -> bool {
    let mut in_fence = false;
    for l in &lines[..start] {
        let t = l.trim_start();
        // naive fence toggle; good enough for detection here
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut t = t;
        while let Some(rest) = t.strip_prefix('>') {
            t = rest.trim_start();
        }
        if t.strip_prefix("[^")
            .and_then(|r| r.split_once("]:"))
            .is_some_and(|(num, _)| num.chars().all(|c| c.is_ascii_digit()))
        {
            return true;
        }
    }
    false
}

/// Convert an ordered list item into a GFM footnote definition.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(replace_footnote_line(" 1. Note"), " [^1]: Note");
/// ```
fn replace_footnote_line(line: &str) -> String {
    FOOTNOTE_LINE_RE
        .replace(line, |caps: &Captures| {
            format!("{}[^{}]: {}", &caps["indent"], &caps["num"], &caps["rest"])
        })
        .to_string()
}

fn convert_block(lines: &mut [String]) {
    let Some((start, end)) = footnote_block_range(lines) else {
        return;
    };
    if !has_h2_heading_before(lines, start) || has_existing_footnote_block(lines, start) {
        return;
    }
    for line in &mut lines[start..end] {
        if FOOTNOTE_LINE_RE.is_match(line) {
            *line = replace_footnote_line(line);
        }
    }
}

fn scan_reference(text: &str, start: usize) -> (Option<(usize, usize, &str)>, usize) {
    let bytes = text.as_bytes();
    let mut idx = start;
    while idx < text.len() {
        let Some(rel_pos) = text[idx..].find("[^") else {
            return (None, text.len());
        };
        let start_idx = idx + rel_pos;
        let number_start = start_idx + 2;
        let mut number_end = number_start;
        while number_end < text.len() && bytes[number_end].is_ascii_digit() {
            number_end += 1;
        }
        if number_end == number_start {
            idx = number_start;
            continue;
        }
        if number_end >= text.len() {
            return (None, text.len());
        }
        if bytes[number_end] != b']' {
            idx = number_end;
            continue;
        }
        let after_bracket = number_end + 1;
        if after_bracket < text.len() && bytes[after_bracket] == b':' {
            idx = after_bracket;
            continue;
        }
        let number = &text[number_start..number_end];
        return (Some((start_idx, after_bracket, number)), after_bracket);
    }
    (None, text.len())
}

fn rewrite_refs_in_segment(text: &str, mapping: &HashMap<String, usize>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    while cursor < text.len() {
        let (maybe_ref, _) = scan_reference(text, cursor);
        if let Some((start, after, number)) = maybe_ref {
            out.push_str(&text[cursor..start]);
            if let Some(&new_number) = mapping.get(number) {
                write!(&mut out, "[^{new_number}]").expect("write to string cannot fail");
            } else {
                out.push_str(&text[start..after]);
            }
            cursor = after;
        } else {
            out.push_str(&text[cursor..]);
            break;
        }
    }
    out
}

fn rewrite_tokens(text: &str, mapping: &HashMap<String, usize>) -> String {
    let mut rewritten = String::with_capacity(text.len());
    for token in tokenize_markdown(text) {
        match token {
            Token::Text(segment) => {
                rewritten.push_str(&rewrite_refs_in_segment(segment, mapping));
            }
            other => push_original_token(&other, &mut rewritten),
        }
    }
    rewritten
}

fn collect_reference_mapping(lines: &[String]) -> HashMap<String, usize> {
    let mut mapping = HashMap::new();
    let mut next = 1;
    for line in lines {
        for token in tokenize_markdown(line) {
            if let Token::Text(text) = token {
                let mut cursor = 0;
                while cursor < text.len() {
                    let (maybe_ref, _) = scan_reference(text, cursor);
                    let Some((_, after, number)) = maybe_ref else {
                        break;
                    };
                    if !mapping.contains_key(number) {
                        mapping.insert(number.to_string(), next);
                        next += 1;
                    }
                    cursor = after;
                }
            }
        }
    }
    mapping
}

#[derive(Clone)]
struct DefinitionLine {
    index: usize,
    new_number: usize,
    line: String,
}

struct DefinitionParts<'a> {
    prefix: &'a str,
    number: &'a str,
    rest: &'a str,
}

fn parse_definition(line: &str) -> Option<DefinitionParts<'_>> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    if len < 5 {
        return None;
    }
    let mut idx = 0;
    while idx < len && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    loop {
        if idx < len && bytes[idx] == b'>' {
            idx += 1;
            while idx < len && bytes[idx].is_ascii_whitespace() {
                idx += 1;
            }
        } else {
            break;
        }
    }
    let prefix_end = idx;
    if idx >= len || bytes[idx] != b'[' {
        return None;
    }
    idx += 1;
    if idx >= len || bytes[idx] != b'^' {
        return None;
    }
    idx += 1;
    let number_start = idx;
    while idx < len && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx == number_start {
        return None;
    }
    if idx >= len || bytes[idx] != b']' {
        return None;
    }
    let number_end = idx;
    idx += 1;
    while idx < len && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    if idx >= len || bytes[idx] != b':' {
        return None;
    }
    idx += 1;
    Some(DefinitionParts {
        prefix: &line[..prefix_end],
        number: &line[number_start..number_end],
        rest: &line[idx..],
    })
}

fn footnote_definition_block_range(lines: &[String]) -> Option<(usize, usize)> {
    let (start, end) = trimmed_range(lines, |line| {
        line.trim().is_empty() || parse_definition(line).is_some()
    });
    if start < end
        && lines[start..end]
            .iter()
            .any(|line| parse_definition(line).is_some())
    {
        Some((start, end))
    } else {
        None
    }
}

fn renumber_footnotes(lines: &mut [String]) {
    let mut mapping = collect_reference_mapping(lines);
    let mut next_number = mapping.len() + 1;
    let mut definitions = Vec::new();
    let mut is_definition = vec![false; lines.len()];
    let numeric_list_range = footnote_block_range(lines);
    let skip_numeric_conversion = numeric_list_range
        .as_ref()
        .is_some_and(|(start, _)| has_existing_footnote_block(lines, *start));

    for (idx, line) in lines.iter().enumerate() {
        if let Some(parts) = parse_definition(line) {
            let new_number = if let Some(&mapped) = mapping.get(parts.number) {
                mapped
            } else {
                let assigned = next_number;
                next_number += 1;
                mapping.insert(parts.number.to_string(), assigned);
                assigned
            };
            let rewritten_rest = rewrite_tokens(parts.rest, &mapping);
            let mut new_line = String::with_capacity(parts.prefix.len() + rewritten_rest.len() + 8);
            new_line.push_str(parts.prefix);
            write!(&mut new_line, "[^{new_number}]:").expect("write to string cannot fail");
            new_line.push_str(&rewritten_rest);
            definitions.push(DefinitionLine {
                index: idx,
                new_number,
                line: new_line,
            });
            is_definition[idx] = true;
        } else if numeric_list_range
            .as_ref()
            .is_some_and(|(start, end)| (*start..*end).contains(&idx))
            && let Some(caps) = FOOTNOTE_LINE_RE.captures(line)
        {
            if skip_numeric_conversion {
                continue;
            }
            if mapping.is_empty() && definitions.is_empty() {
                continue;
            }
            let number = &caps["num"];
            let new_number = if let Some(&mapped) = mapping.get(number) {
                mapped
            } else {
                let assigned = next_number;
                next_number += 1;
                mapping.insert(number.to_string(), assigned);
                assigned
            };
            let indent = &caps["indent"];
            let rest = &caps["rest"];
            let rewritten_rest = rewrite_tokens(rest, &mapping);
            let mut new_line = String::with_capacity(indent.len() + rewritten_rest.len() + 8);
            new_line.push_str(indent);
            write!(&mut new_line, "[^{new_number}]:").expect("write to string cannot fail");
            new_line.push(' ');
            new_line.push_str(&rewritten_rest);
            definitions.push(DefinitionLine {
                index: idx,
                new_number,
                line: new_line,
            });
            is_definition[idx] = true;
        }
    }

    if mapping.is_empty() && definitions.is_empty() {
        return;
    }

    for (idx, line) in lines.iter_mut().enumerate() {
        if is_definition[idx] {
            continue;
        }
        let rewritten = rewrite_tokens(line, &mapping);
        *line = rewritten;
    }

    if definitions.is_empty() {
        return;
    }

    for def in &definitions {
        lines[def.index].clone_from(&def.line);
    }

    if let Some((start, end)) = footnote_definition_block_range(lines) {
        let mut defs_in_block: Vec<&DefinitionLine> = definitions
            .iter()
            .filter(|d| (start..end).contains(&d.index))
            .collect();
        if defs_in_block.len() > 1 {
            defs_in_block
                .sort_by(|a, b| a.new_number.cmp(&b.new_number).then(a.index.cmp(&b.index)));
            let positions: Vec<usize> = (start..end)
                .filter(|&idx| parse_definition(&lines[idx]).is_some())
                .collect();
            for (pos, def) in positions.into_iter().zip(defs_in_block.into_iter()) {
                lines[pos].clone_from(&def.line);
            }
        }
    }
}

#[inline]
fn is_atx_heading_prefix(s: &str) -> bool {
    ATX_HEADING_RE.is_match(s)
}

/// Convert bare numeric footnote references to Markdown footnote syntax.
#[must_use]
pub fn convert_footnotes(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());

    for line in lines {
        if is_atx_heading_prefix(line) {
            out.push(line.clone());
        } else {
            let mut converted = String::with_capacity(line.len());
            for token in tokenize_markdown(line) {
                match token {
                    Token::Text(t) => converted.push_str(&convert_inline(t)),
                    other => push_original_token(&other, &mut converted),
                }
            }
            out.push(converted);
        }
    }

    convert_block(&mut out);
    renumber_footnotes(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_inline_numbers() {
        let input = vec!["See the docs.2".to_string()];
        let expected = vec!["See the docs.[^1]".to_string()];
        assert_eq!(convert_footnotes(&input), expected);
    }

    #[test]
    fn converts_final_list() {
        let input = vec![
            "Text.".to_string(),
            String::new(),
            "## Footnotes".to_string(),
            String::new(),
            " 1. First".to_string(),
            " 2. Second".to_string(),
        ];
        let expected = vec![
            "Text.".to_string(),
            String::new(),
            "## Footnotes".to_string(),
            String::new(),
            " [^1]: First".to_string(),
            " [^2]: Second".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), expected);
    }

    #[test]
    fn converts_list_with_blank_lines() {
        let input = vec![
            "Text.".to_string(),
            String::new(),
            "## Footnotes".to_string(),
            String::new(),
            " 1. First".to_string(),
            String::new(),
            " 2. Second".to_string(),
            String::new(),
            "10. Tenth".to_string(),
        ];
        let expected = vec![
            "Text.".to_string(),
            String::new(),
            "## Footnotes".to_string(),
            String::new(),
            " [^1]: First".to_string(),
            String::new(),
            " [^2]: Second".to_string(),
            String::new(),
            "[^3]: Tenth".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), expected);
    }

    #[test]
    fn idempotent_on_existing_block() {
        let input = vec![" [^1]: First".to_string()];
        assert_eq!(convert_footnotes(&input), input);
    }

    #[test]
    fn skips_with_existing_block() {
        let input = vec![
            "[^1]: Old".to_string(),
            "## Footnotes".to_string(),
            " 2. New".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), input);
    }

    #[test]
    fn skips_without_h2() {
        let input = vec!["Text.".to_string(), " 1. First".to_string()];
        assert_eq!(convert_footnotes(&input), input);
    }

    #[test]
    fn skips_when_list_not_last() {
        let input = vec![
            "## Footnotes".to_string(),
            " 1. First".to_string(),
            String::new(),
            "Tail.".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), input);
    }

    #[test]
    fn skips_when_block_has_only_blanks() {
        let input = vec!["## Footnotes".to_string(), String::new()];
        assert_eq!(convert_footnotes(&input), input);
    }

    #[test]
    fn multiple_inline_notes_in_one_line() {
        let input = vec!["First.1 Then?2".to_string()];
        let expected = vec!["First.[^1] Then?[^2]".to_string()];
        assert_eq!(convert_footnotes(&input), expected);
    }

    #[test]
    fn ignores_non_numeric_footnote_block() {
        let input = vec!["Text.".to_string(), " a. note".to_string()];
        assert_eq!(convert_footnotes(&input), input);
    }

    #[test]
    fn empty_input_returns_empty_vec() {
        let input: Vec<String> = Vec::new();
        assert!(convert_footnotes(&input).is_empty());
    }

    #[test]
    fn converts_only_final_contiguous_block() {
        let input = vec![
            "Intro.".to_string(),
            "1. not a footnote".to_string(),
            "More text.".to_string(),
            "## Footnotes".to_string(),
            "2. final".to_string(),
        ];
        let expected = vec![
            "Intro.".to_string(),
            "1. not a footnote".to_string(),
            "More text.".to_string(),
            "## Footnotes".to_string(),
            "[^1]: final".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), expected);
    }
    #[test]
    fn renumbers_references_and_definitions() {
        let input = vec![
            "First reference.[^7]".to_string(),
            "Second reference.[^3]".to_string(),
            String::new(),
            "  [^3]: Third footnote".to_string(),
            "  [^7]: Seventh footnote".to_string(),
        ];
        let expected = vec![
            "First reference.[^1]".to_string(),
            "Second reference.[^2]".to_string(),
            String::new(),
            "  [^1]: Seventh footnote".to_string(),
            "  [^2]: Third footnote".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), expected);
    }

    #[test]
    fn assigns_new_numbers_to_unreferenced_definitions() {
        let input = vec![
            "Alpha.[^5]".to_string(),
            "Beta.[^2]".to_string(),
            String::new(),
            "[^1]: Legacy footnote".to_string(),
            "[^2]: Beta footnote".to_string(),
            "[^5]: Alpha footnote".to_string(),
        ];
        let expected = vec![
            "Alpha.[^1]".to_string(),
            "Beta.[^2]".to_string(),
            String::new(),
            "[^1]: Alpha footnote".to_string(),
            "[^2]: Beta footnote".to_string(),
            "[^3]: Legacy footnote".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), expected);
    }

    #[test]
    fn updates_references_inside_definitions() {
        let input = vec![
            "Intro.[^4]".to_string(),
            String::new(),
            "[^4]: See [^2] for context".to_string(),
            "[^2]: Base note".to_string(),
        ];
        let expected = vec![
            "Intro.[^1]".to_string(),
            String::new(),
            "[^1]: See [^2] for context".to_string(),
            "[^2]: Base note".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), expected);
    }

    #[test]
    fn renumbers_numeric_list_without_heading() {
        let input = vec![
            "First reference.[^7]".to_string(),
            "Second reference.[^3]".to_string(),
            String::new(),
            "1. Legacy footnote".to_string(),
            "3. Third footnote".to_string(),
            "7. Seventh footnote".to_string(),
        ];
        let expected = vec![
            "First reference.[^1]".to_string(),
            "Second reference.[^2]".to_string(),
            String::new(),
            "[^1]: Seventh footnote".to_string(),
            "[^2]: Third footnote".to_string(),
            "[^3]: Legacy footnote".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), expected);
    }

    #[test]
    fn leaves_numeric_list_without_references_unchanged() {
        let input = vec![
            "Ordinary list:".to_string(),
            "1. Apples".to_string(),
            "2. Bananas".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), input);
    }
}
