//! Footnote normalisation utilities.
//!
//! Converts bare numeric references in text to GitHub-flavoured Markdown
//! footnote links and normalises footnote numbering and ordering.
//!
//! The conversion operates in two phases:
//!
//! 1. **Reference normalisation**: Converts inline numeric references (e.g.,
//!    `.1` becomes `.[^1]`) and rewrites eligible trailing numeric lists into
//!    footnote definition blocks. List conversion requires the list immediately
//!    follows an H2 heading with no existing definitions earlier in the document.
//!
//! 2. **Sequential renumbering**: Scans the document for footnote references in
//!    encounter order, assigns sequential identifiers starting from 1, and
//!    rewrites both inline references and their matching definitions. Sorts
//!    definition blocks by the new sequential identifiers whilst preserving
//!    multi-line definition content and converting eligible numeric lists
//!    when at least one footnote reference exists.

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::LazyLock;

use regex::{Captures, Match, Regex};

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

static FOOTNOTE_REF_RE: LazyLock<Regex> = lazy_regex!(
    r"\[\^(?P<num>\d+)\]",
    "footnote reference pattern should compile",
);

static DEF_RE: LazyLock<Regex> = lazy_regex!(
    r"^(?P<prefix>(?:\s*>\s*)*\s*)\[\^(?P<num>\d+)\]\s*:(?P<rest>.*)$",
    "footnote definition pattern should compile",
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
            let mat = caps.get(0).expect("regex matched without capture");
            let match_str = mat.as_str();
            let num_match = caps.name("num").expect("regex matched without num capture");
            let style_start = caps
                .name("style")
                .map_or(num_match.start() - mat.start(), |m| m.start() - mat.start());
            let whitespace = &match_str[pre.len()..style_start];
            let leading = if mat.start() == 0 { whitespace } else { "" };
            format!("{pre}{leading}{style}[^{num}]:{colons}{boundary}")
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
            let num_match = caps
                .name("num")
                .expect("footnote line capture missing number");
            let rest_match = caps
                .name("rest")
                .expect("footnote line capture missing rest");
            let whitespace = &line[num_match.end() + 1..rest_match.start()];
            format!(
                "{}[^{}]:{}{}",
                &caps["indent"], &caps["num"], whitespace, &caps["rest"]
            )
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

/// Check whether the text before a reference contains only indentation or
/// blockquote markers.
fn matches_definition_prefix(prefix: &str) -> bool {
    let mut remaining = prefix;
    loop {
        remaining = remaining.trim_start_matches(char::is_whitespace);
        if remaining.is_empty() {
            return true;
        }
        if let Some(stripped) = remaining.strip_prefix('>') {
            remaining = stripped;
            continue;
        }
        return false;
    }
}

/// Determine whether a matched reference is part of a definition header.
fn is_definition_like(text: &str, mat: &Match) -> bool {
    if !matches_definition_prefix(&text[..mat.start()]) {
        return false;
    }
    let suffix = &text[mat.end()..];
    let trimmed = suffix.trim_start_matches(char::is_whitespace);
    if !trimmed.starts_with(':') {
        return false;
    }
    let after_colon = &trimmed[1..];
    if suffix.len() == trimmed.len() && after_colon.starts_with(':') {
        return false;
    }
    true
}

fn rewrite_refs_in_segment(text: &str, mapping: &HashMap<usize, usize>) -> String {
    FOOTNOTE_REF_RE
        .replace_all(text, |caps: &Captures| {
            let mat = caps.get(0).expect("regex matched without capture");
            if is_definition_like(text, &mat) {
                return caps[0].to_string();
            }
            caps["num"]
                .parse::<usize>()
                .ok()
                .and_then(|number| mapping.get(&number).copied())
                .map_or_else(
                    || caps[0].to_string(),
                    |new_number| format!("[^{new_number}]"),
                )
        })
        .into_owned()
}

fn rewrite_tokens(text: &str, mapping: &HashMap<usize, usize>) -> String {
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

fn collect_reference_mapping(lines: &[String]) -> HashMap<usize, usize> {
    let mut mapping = HashMap::new();
    let mut next = 1;
    for line in lines {
        for token in tokenize_markdown(line) {
            if let Token::Text(text) = token {
                for caps in FOOTNOTE_REF_RE.captures_iter(text) {
                    let Some(mat) = caps.get(0) else {
                        continue;
                    };
                    if is_definition_like(text, &mat) {
                        continue;
                    }
                    if let Ok(number) = caps["num"].parse::<usize>() {
                        if mapping.contains_key(&number) {
                            continue;
                        }
                        mapping.insert(number, next);
                        next += 1;
                    }
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
    number: usize,
    rest: &'a str,
}

fn parse_definition(line: &str) -> Option<DefinitionParts<'_>> {
    DEF_RE.captures(line).and_then(|caps| {
        let number = caps["num"].parse::<usize>().ok()?;
        Some(DefinitionParts {
            prefix: caps.name("prefix").map_or("", |m| m.as_str()),
            number,
            rest: caps.name("rest").map_or("", |m| m.as_str()),
        })
    })
}

fn is_definition_continuation(line: &str) -> bool {
    line.chars().next().is_some_and(char::is_whitespace)
}

fn footnote_definition_block_range(lines: &[String]) -> Option<(usize, usize)> {
    let (mut start, end) = trimmed_range(lines, |line| {
        line.trim().is_empty()
            || parse_definition(line).is_some()
            || is_definition_continuation(line)
    });
    while start < end
        && parse_definition(&lines[start]).is_none()
        && !lines[start].trim().is_empty()
    {
        start += 1;
    }
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

fn definition_segment_end(lines: &[String], start: usize, block_end: usize) -> usize {
    let mut idx = start + 1;
    while idx < block_end {
        let line = &lines[idx];
        if parse_definition(line).is_some() {
            break;
        }
        if is_definition_continuation(line) {
            idx += 1;
            continue;
        }
        if line.trim().is_empty() {
            if idx + 1 < block_end && parse_definition(&lines[idx + 1]).is_some() {
                break;
            }
            idx += 1;
            continue;
        }
        break;
    }
    idx
}

/// Keep multi-line footnote definitions attached to their bodies when sorting.
///
/// This collects each definition together with its continuation lines, orders
/// the segments by the new footnote numbers, and writes the reordered block
/// back to `lines`.
///
/// # Examples
///
/// ```ignore
/// let mut lines = vec![
///     "[^2]: Second".to_string(),
///     "    Follow up.".to_string(),
///     "[^1]: First".to_string(),
/// ];
/// let defs = vec![
///     DefinitionLine {
///         index: 0,
///         new_number: 2,
///         line: "[^2]: Second".to_string(),
///     },
///     DefinitionLine {
///         index: 2,
///         new_number: 1,
///         line: "[^1]: First".to_string(),
///     },
/// ];
/// reorder_definition_block(&mut lines, 0, lines.len(), &defs);
/// assert_eq!(lines[0], "[^1]: First");
/// ```
///
/// (Simplified example; comprehensive coverage lives in unit tests.)
fn reorder_definition_block(
    lines: &mut [String],
    start: usize,
    end: usize,
    definitions: &[DefinitionLine],
) {
    let header_positions: Vec<usize> = (start..end)
        .filter(|&idx| parse_definition(&lines[idx]).is_some())
        .collect();
    if header_positions.len() <= 1 {
        return;
    }

    let def_lookup: HashMap<usize, &DefinitionLine> = definitions
        .iter()
        .filter(|definition| (start..end).contains(&definition.index))
        .map(|definition| (definition.index, definition))
        .collect();
    if def_lookup.len() <= 1 {
        return;
    }

    let prefix_len = header_positions.first().map_or(0, |first| first - start);
    let mut segments: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut consumed = start + prefix_len;
    for &position in &header_positions {
        let mut leading_start = position;
        while leading_start > consumed
            && lines[leading_start - 1].trim().is_empty()
            && !is_definition_continuation(&lines[leading_start - 1])
        {
            leading_start -= 1;
        }
        let next_bound = definition_segment_end(lines, position, end);
        if let Some(definition) = def_lookup.get(&position) {
            let mut segment = lines[leading_start..next_bound].to_vec();
            if segment.is_empty() {
                segment.push(definition.line.clone());
            } else {
                let header_index = position - leading_start;
                segment[header_index].clone_from(&definition.line);
            }
            segments.push((definition.new_number, definition.index, segment));
        }
        consumed = next_bound;
    }

    if segments.len() <= 1 {
        return;
    }

    segments.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    let mut first_leading = Vec::new();
    if let Some((_, _, first_segment)) = segments.first_mut() {
        while first_segment
            .first()
            .is_some_and(|line| line.trim().is_empty() && !is_definition_continuation(line))
        {
            first_leading.push(first_segment.remove(0));
        }
    }

    let mut reordered = Vec::with_capacity(end - start);
    if prefix_len > 0 {
        reordered.extend(lines[start..start + prefix_len].iter().cloned());
    }

    for (idx, (_, _, segment)) in segments.into_iter().enumerate() {
        reordered.extend(segment);
        if idx == 0 && !first_leading.is_empty() {
            reordered.append(&mut first_leading);
        }
    }

    if reordered.len() == end - start {
        lines[start..end].clone_from_slice(&reordered);
    }
}

struct DefinitionUpdates {
    definitions: Vec<DefinitionLine>,
    is_definition_line: Vec<bool>,
}

fn assign_new_number(
    mapping: &mut HashMap<usize, usize>,
    number: usize,
    next_number: &mut usize,
) -> usize {
    if let Some(&mapped) = mapping.get(&number) {
        mapped
    } else {
        let assigned = *next_number;
        *next_number += 1;
        mapping.insert(number, assigned);
        assigned
    }
}

fn should_convert_numeric_line(
    index: usize,
    numeric_range: Option<(usize, usize)>,
    skip_numeric_conversion: bool,
) -> bool {
    if skip_numeric_conversion {
        return false;
    }
    numeric_range.is_some_and(|(start, end)| index >= start && index < end)
}

fn collect_definition_updates(
    lines: &[String],
    mapping: &mut HashMap<usize, usize>,
) -> DefinitionUpdates {
    let mut next_number = mapping.len() + 1;
    let mut definitions = Vec::new();
    let mut is_definition_line = vec![false; lines.len()];
    let numeric_list_range = footnote_block_range(lines);
    let skip_numeric_conversion = numeric_list_range
        .as_ref()
        .is_some_and(|(start, _)| has_existing_footnote_block(lines, *start));

    for (idx, line) in lines.iter().enumerate() {
        if let Some(parts) = parse_definition(line) {
            let new_number = assign_new_number(mapping, parts.number, &mut next_number);
            let rewritten_rest = rewrite_tokens(parts.rest, mapping);
            let mut new_line = String::with_capacity(parts.prefix.len() + rewritten_rest.len() + 8);
            new_line.push_str(parts.prefix);
            write!(&mut new_line, "[^{new_number}]:").expect("write to string cannot fail");
            new_line.push_str(&rewritten_rest);
            definitions.push(DefinitionLine {
                index: idx,
                new_number,
                line: new_line,
            });
            is_definition_line[idx] = true;
        } else if should_convert_numeric_line(idx, numeric_list_range, skip_numeric_conversion)
            && let Some(caps) = FOOTNOTE_LINE_RE.captures(line)
        {
            if mapping.is_empty() && definitions.is_empty() {
                continue;
            }
            let Ok(number) = caps["num"].parse::<usize>() else {
                continue;
            };
            let new_number = assign_new_number(mapping, number, &mut next_number);
            let indent = caps.name("indent").map_or("", |m| m.as_str());
            let rest = caps.name("rest").map_or("", |m| m.as_str());
            let rewritten_rest = rewrite_tokens(rest, mapping);
            let num_match = caps
                .name("num")
                .expect("numeric list capture missing number");
            let rest_match = caps
                .name("rest")
                .expect("numeric list capture missing rest");
            let whitespace = &line[num_match.end() + 1..rest_match.start()];
            let mut new_line =
                String::with_capacity(indent.len() + rewritten_rest.len() + whitespace.len() + 8);
            new_line.push_str(indent);
            write!(&mut new_line, "[^{new_number}]:").expect("write to string cannot fail");
            new_line.push_str(whitespace);
            new_line.push_str(&rewritten_rest);
            definitions.push(DefinitionLine {
                index: idx,
                new_number,
                line: new_line,
            });
            is_definition_line[idx] = true;
        }
    }

    DefinitionUpdates {
        definitions,
        is_definition_line,
    }
}

fn apply_mapping_to_lines(
    lines: &mut [String],
    mapping: &HashMap<usize, usize>,
    is_definition_line: &[bool],
) {
    for (idx, line) in lines.iter_mut().enumerate() {
        if is_definition_line.get(idx).copied().unwrap_or(false) {
            continue;
        }
        *line = rewrite_tokens(line, mapping);
    }
}

fn rewrite_definition_headers(lines: &mut [String], definitions: &[DefinitionLine]) {
    for definition in definitions {
        lines[definition.index].clone_from(&definition.line);
    }
}

fn renumber_footnotes(lines: &mut [String]) {
    let mut mapping = collect_reference_mapping(lines);
    let DefinitionUpdates {
        definitions,
        is_definition_line,
    } = collect_definition_updates(lines, &mut mapping);

    if mapping.is_empty() && definitions.is_empty() {
        return;
    }

    apply_mapping_to_lines(lines, &mapping, &is_definition_line);

    if definitions.is_empty() {
        return;
    }

    rewrite_definition_headers(lines, &definitions);

    if let Some((start, end)) = footnote_definition_block_range(lines) {
        reorder_definition_block(lines, start, end, &definitions);
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
    fn preserves_multiline_definition_blocks() {
        let input = vec![
            "Intro.[^2]".to_string(),
            String::new(),
            "[^1]: Legacy footnote".to_string(),
            "    More legacy context.".to_string(),
            String::new(),
            "[^2]: Current footnote".to_string(),
            "    Additional context.".to_string(),
        ];
        let expected = vec![
            "Intro.[^1]".to_string(),
            String::new(),
            "[^1]: Current footnote".to_string(),
            "    Additional context.".to_string(),
            String::new(),
            "[^2]: Legacy footnote".to_string(),
            "    More legacy context.".to_string(),
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
