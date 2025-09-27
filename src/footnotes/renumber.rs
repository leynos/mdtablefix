//! Sequential renumbering of footnote references and definitions.

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::LazyLock;

use regex::{Captures, Match, Regex};

use crate::textproc::{Token, push_original_token, tokenize_markdown};

use super::lists::{footnote_block_range, has_existing_footnote_block, trimmed_range};
use super::parsing::{FOOTNOTE_LINE_RE, is_definition_continuation, parse_definition};

static FOOTNOTE_REF_RE: LazyLock<Regex> = lazy_regex!(
    r"\[\^(?P<num>\d+)\]",
    "footnote reference pattern should compile",
);

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

fn is_definition_like(text: &str, mat: &Match) -> bool {
    if !matches_definition_prefix(&text[..mat.start()]) {
        return false;
    }
    let suffix = &text[mat.end()..];
    let trimmed = suffix.trim_start_matches(char::is_whitespace);
    if !trimmed.starts_with(':') {
        return false;
    }
    if suffix.len() == trimmed.len() && trimmed.starts_with("::") {
        return false;
    }
    parse_definition(text.trim_end()).is_some()
}

fn is_fence_line(line: &str) -> bool {
    let mut trimmed = line.trim_start();
    while let Some(rest) = trimmed.strip_prefix('>') {
        trimmed = rest.trim_start();
    }
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
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
    let mut in_fence = false;
    for line in lines {
        if is_fence_line(line) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
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

struct NumericCandidate {
    index: usize,
    number: usize,
    indent: String,
    whitespace: String,
    rest: String,
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
    let mut next_number = mapping.values().copied().max().unwrap_or(0) + 1;
    let mut definitions = Vec::new();
    let mut is_definition_line = vec![false; lines.len()];
    let mut numeric_candidates: Vec<NumericCandidate> = Vec::new();
    let numeric_list_range = footnote_block_range(lines);
    let skip_numeric_conversion = numeric_list_range
        .as_ref()
        .is_some_and(|(start, _)| has_existing_footnote_block(lines, *start));

    let mut in_fence = false;

    for (idx, line) in lines.iter().enumerate() {
        if is_fence_line(line) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

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
            let indent = caps.name("indent").map_or("", |m| m.as_str()).to_string();
            let rest = caps.name("rest").map_or("", |m| m.as_str()).to_string();
            let num_match = caps
                .name("num")
                .expect("numeric list capture missing number");
            let rest_match = caps
                .name("rest")
                .expect("numeric list capture missing rest");
            let whitespace = line[num_match.end() + 1..rest_match.start()].to_string();
            numeric_candidates.push(NumericCandidate {
                index: idx,
                number,
                indent,
                whitespace,
                rest,
            });
        }
    }

    for candidate in numeric_candidates.into_iter().rev() {
        let new_number = assign_new_number(mapping, candidate.number, &mut next_number);
        let rewritten_rest = rewrite_tokens(&candidate.rest, mapping);
        let mut new_line = String::with_capacity(
            candidate.indent.len() + candidate.whitespace.len() + rewritten_rest.len() + 8,
        );
        new_line.push_str(&candidate.indent);
        write!(&mut new_line, "[^{new_number}]:").expect("write to string cannot fail");
        new_line.push_str(&candidate.whitespace);
        new_line.push_str(&rewritten_rest);
        definitions.push(DefinitionLine {
            index: candidate.index,
            new_number,
            line: new_line,
        });
        is_definition_line[candidate.index] = true;
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
    let mut in_fence = false;
    for (idx, line) in lines.iter_mut().enumerate() {
        if is_fence_line(line) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || is_definition_line.get(idx).copied().unwrap_or(false) {
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

pub(super) fn renumber_footnotes(lines: &mut [String]) {
    let mut mapping = collect_reference_mapping(lines);
    let DefinitionUpdates {
        definitions,
        is_definition_line,
    } = collect_definition_updates(lines, &mut mapping);

    if mapping.is_empty() && definitions.is_empty() {
        return;
    }

    if definitions.is_empty() && lines.iter().any(|line| FOOTNOTE_LINE_RE.is_match(line)) {
        return;
    }

    apply_mapping_to_lines(lines, &mapping, &is_definition_line);

    rewrite_definition_headers(lines, &definitions);

    if let Some((start, end)) = footnote_definition_block_range(lines) {
        reorder_definition_block(lines, start, end, &definitions);
    }
}
