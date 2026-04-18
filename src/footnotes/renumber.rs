//! Sequential renumbering of footnote references and definitions.

use std::{collections::HashMap, sync::LazyLock};

use regex::{Captures, Match, Regex};

mod definitions;

#[cfg(test)]
use definitions::numeric_candidate_from_line;
use definitions::{
    DefinitionUpdates,
    collect_definition_updates,
    reorder_definition_block,
    rewrite_definition_headers,
};

use super::{
    lists::{footnote_block_range, has_existing_footnote_block, trimmed_range},
    parsing::{FOOTNOTE_LINE_RE, is_definition_continuation, parse_definition},
};
use crate::textproc::{Token, push_original_token, tokenize_markdown};

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
                collect_reference_mapping_from_text(text, &mut mapping, &mut next);
            }
        }
    }
    mapping
}

fn collect_reference_mapping_from_text(
    text: &str,
    mapping: &mut HashMap<usize, usize>,
    next: &mut usize,
) {
    for caps in FOOTNOTE_REF_RE.captures_iter(text) {
        let Some(mat) = caps.get(0) else {
            continue;
        };
        if is_definition_like(text, &mat) {
            continue;
        }
        let Ok(number) = caps["num"].parse::<usize>() else {
            continue;
        };
        if mapping.contains_key(&number) {
            continue;
        }
        mapping.insert(number, *next);
        *next += 1;
    }
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

#[cfg(test)]
mod tests;
