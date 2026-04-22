//! Inline wrapping helpers that keep code spans intact.
//!
//! These functions operate on token streams so `wrap_text` can preserve
//! inline code, links, and trailing punctuation without reimplementing the
//! grouping logic in multiple places.

use std::ops::Range;

use textwrap::{core::Fragment, wrap_algorithms::wrap_first_fit};
use unicode_width::UnicodeWidthStr;

use super::tokenize;

#[derive(Copy, Clone, PartialEq, Eq)]
enum SpanKind {
    General,
    Code,
    Link,
}

#[inline]
fn is_trailing_punct(c: char) -> bool {
    // ASCII closers + common Unicode closers and word-final punctuation
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '"' | '\''
    ) || "…—–»›）］】》」』、。，：；！？”.’".contains(c)
}

fn looks_like_link(token: &str) -> bool {
    (token.starts_with('[') || token.starts_with("!["))
        && token.contains("](")
        && token.ends_with(')')
}

fn is_whitespace_token(token: &str) -> bool { token.chars().all(char::is_whitespace) }

fn is_inline_code_token(token: &str) -> bool { token.starts_with('`') && token.ends_with('`') }

fn extend_punctuation(tokens: &[String], mut j: usize, width: &mut usize) -> usize {
    while j < tokens.len() && tokens[j].chars().all(is_trailing_punct) {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
    }
    j
}

/// Decide whether whitespace between grouped tokens should stay attached to the
/// current span.
///
/// Links absorb following whitespace when another link, inline code span, or
/// punctuation immediately follows so that rendered Markdown keeps those items
/// together. Code spans are only coupled with trailing punctuation so that two
/// adjacent code spans can break across lines, but `code`, style suffixes still
/// cling to the preceding span.
fn should_couple_whitespace(kind: SpanKind, next_token: Option<&String>) -> bool {
    match (kind, next_token) {
        (SpanKind::Link, Some(next))
            if looks_like_link(next)
                || is_inline_code_token(next)
                || next.chars().all(is_trailing_punct) =>
        {
            true
        }
        (SpanKind::Code, Some(next)) if next.chars().all(is_trailing_punct) => true,
        _ => false,
    }
}

#[inline]
fn merge_code_span(tokens: &[String], i: usize, width: &mut usize) -> usize {
    debug_assert!(
        tokens[i] == "`",
        "merge_code_span requires a single backtick opener"
    );
    let mut j = i + 1;
    while j < tokens.len() && tokens[j] != "`" {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
    }
    if j < tokens.len() {
        *width += UnicodeWidthStr::width(tokens[j].as_str());
        j += 1;
        j = extend_punctuation(tokens, j, width);
    }
    j
}

pub(super) fn determine_token_span(tokens: &[String], start: usize) -> (usize, usize) {
    let mut end = start + 1;
    let mut width = UnicodeWidthStr::width(tokens[start].as_str());
    let mut kind = SpanKind::General;

    if tokens[start] == "`" {
        kind = SpanKind::Code;
        end = merge_code_span(tokens, start, &mut width);
    } else if is_inline_code_token(&tokens[start]) {
        kind = SpanKind::Code;
        end = extend_punctuation(tokens, end, &mut width);
    } else if looks_like_link(&tokens[start]) {
        kind = SpanKind::Link;
        end = extend_punctuation(tokens, end, &mut width);
    }

    while end < tokens.len() {
        let token = &tokens[end];
        if is_whitespace_token(token) {
            if should_couple_whitespace(kind, tokens.get(end + 1)) {
                width += UnicodeWidthStr::width(token.as_str());
                end += 1;
                continue;
            }

            break;
        }

        if token.chars().all(is_trailing_punct) {
            if matches!(kind, SpanKind::Code | SpanKind::Link) {
                width += UnicodeWidthStr::width(token.as_str());
                end += 1;
                continue;
            }
            break;
        }

        let is_link = looks_like_link(token);
        let is_code = is_inline_code_token(token);

        if kind == SpanKind::Link && is_link {
            width += UnicodeWidthStr::width(token.as_str());
            end += 1;
            end = extend_punctuation(tokens, end, &mut width);
            continue;
        }

        if kind == SpanKind::Code && is_code {
            width += UnicodeWidthStr::width(token.as_str());
            end += 1;
            end = extend_punctuation(tokens, end, &mut width);
            continue;
        }

        break;
    }

    (end, width)
}

#[cfg(test)]
pub(super) fn attach_punctuation_to_previous_line(
    lines: &mut [String],
    current: &str,
    token: &str,
) -> bool {
    if !current.is_empty() || token.len() != 1 || !".?!,:;".contains(token) {
        return false;
    }

    let Some(last_line) = lines.last_mut() else {
        return false;
    };

    if last_line.trim_end().ends_with('`') {
        last_line.push_str(token);
        return true;
    }

    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InlineFragment {
    text: String,
    width: usize,
}

impl InlineFragment {
    fn new(text: String) -> Self {
        let width = UnicodeWidthStr::width(text.as_str());
        Self { text, width }
    }
}

impl Fragment for InlineFragment {
    fn width(&self) -> f64 { width_as_f64(self.width) }

    fn whitespace_width(&self) -> f64 { 0.0 }

    fn penalty_width(&self) -> f64 { 0.0 }
}

fn width_as_f64(width: usize) -> f64 {
    let width = u32::try_from(width).unwrap_or(u32::MAX);
    f64::from(width)
}

fn push_span_text(text: &mut String, tokens: &[String], span: Range<usize>) {
    for token in &tokens[span] {
        if token.len() == 1 && ".?!,:;".contains(token) && text.trim_end().ends_with('`') {
            text.truncate(text.trim_end_matches(char::is_whitespace).len());
        }
        text.push_str(token);
    }
}

fn build_fragments(tokens: &[String]) -> Vec<InlineFragment> {
    let mut fragments: Vec<InlineFragment> = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let (group_end, _) = determine_token_span(tokens, i);
        let span = i..group_end;
        let span_is_whitespace = tokens[span.clone()]
            .iter()
            .all(|token| is_whitespace_token(token));

        if span_is_whitespace {
            let whitespace = tokens[span.clone()].join("");
            fragments.push(InlineFragment::new(whitespace));
            i = group_end;
            continue;
        }

        let mut text = String::new();
        push_span_text(&mut text, tokens, span);
        fragments.push(InlineFragment::new(text));
        i = group_end;
    }

    fragments
}

fn merge_whitespace_only_lines(lines: &[Vec<InlineFragment>]) -> Vec<Vec<InlineFragment>> {
    let mut merged: Vec<Vec<InlineFragment>> = Vec::with_capacity(lines.len());
    let mut pending_whitespace: Vec<InlineFragment> = Vec::new();

    for (index, mut line) in lines.iter().cloned().enumerate() {
        let is_whitespace_only = line
            .iter()
            .all(|fragment| fragment.text.chars().all(char::is_whitespace));

        if is_whitespace_only {
            let next_starts_atomic = lines
                .get(index + 1)
                .and_then(|next_line| next_line.first())
                .is_some_and(|fragment| {
                    is_inline_code_token(fragment.text.as_str())
                        || looks_like_link(fragment.text.as_str())
                });
            let line_is_single_space = line
                .iter()
                .map(|fragment| fragment.text.as_str())
                .collect::<String>()
                == " ";
            let previous_line_has_single_fragment = merged
                .last()
                .is_some_and(|previous_line| previous_line.len() == 1);
            let mut should_carry_whitespace = !line_is_single_space;

            if line_is_single_space
                && !next_starts_atomic
                && let Some(previous_line) = merged.last_mut()
            {
                let should_move_previous_atomic = previous_line
                    .last()
                    .is_some_and(|fragment| is_inline_code_token(fragment.text.as_str()));
                if should_move_previous_atomic {
                    let previous_atomic = previous_line
                        .pop()
                        .expect("line with an atomic tail contains that fragment");
                    pending_whitespace.push(previous_atomic);
                    if previous_line.is_empty() {
                        merged.pop();
                    }
                    should_carry_whitespace = true;
                }
            }

            if line_is_single_space && previous_line_has_single_fragment {
                should_carry_whitespace = true;
            }

            if should_carry_whitespace {
                pending_whitespace.extend(line);
            }
            continue;
        }

        if pending_whitespace.is_empty() {
            merged.push(line);
        } else {
            pending_whitespace.append(&mut line);
            merged.push(std::mem::take(&mut pending_whitespace));
        }
    }

    if !pending_whitespace.is_empty() {
        if let Some(last_line) = merged.last_mut() {
            last_line.append(&mut pending_whitespace);
        } else {
            merged.push(pending_whitespace);
        }
    }

    merged
}

fn rebalance_atomic_tails(lines: &mut [Vec<InlineFragment>]) {
    for index in 0..lines.len().saturating_sub(1) {
        let next_starts_with_single_space = lines[index + 1]
            .first()
            .is_some_and(|fragment| fragment.text == " ");
        let next_continues_with_plain_text = lines[index + 1].get(1).is_some_and(|fragment| {
            !fragment.text.chars().all(char::is_whitespace)
                && !is_inline_code_token(fragment.text.as_str())
                && !looks_like_link(fragment.text.as_str())
        });

        if !next_starts_with_single_space || !next_continues_with_plain_text {
            continue;
        }

        let should_move_atomic_tail = lines[index]
            .last()
            .is_some_and(|fragment| is_inline_code_token(fragment.text.as_str()));
        let should_move_plain_tail = lines[index].len() > 1
            && lines[index].last().is_some_and(|fragment| {
                !fragment.text.chars().all(char::is_whitespace)
                    && !is_inline_code_token(fragment.text.as_str())
                    && !looks_like_link(fragment.text.as_str())
            });

        if should_move_atomic_tail || should_move_plain_tail {
            let trailing_fragment = lines[index]
                .pop()
                .expect("line selected for tail rebalancing contains a trailing fragment");
            lines[index + 1].insert(0, trailing_fragment);
        }
    }
}

fn render_line(line: &[InlineFragment], is_final_output_line: bool) -> String {
    let mut text = line
        .iter()
        .map(|fragment| fragment.text.as_str())
        .collect::<String>();

    if !is_final_output_line && text.ends_with(' ') && !text.ends_with("  ") {
        text.pop();
    }

    text
}

pub(super) fn wrap_preserving_code(text: &str, width: usize) -> Vec<String> {
    let tokens = tokenize::segment_inline(text);
    if tokens.is_empty() {
        return Vec::new();
    }

    let fragments = build_fragments(&tokens);
    let mut lines = Vec::new();
    let mut buffer: Vec<InlineFragment> = Vec::new();

    for fragment in fragments {
        let mut candidate = buffer.clone();
        candidate.push(fragment);
        let wrapped = wrap_first_fit(&candidate, &[width_as_f64(width)]);
        let raw_lines = wrapped.iter().map(|line| line.to_vec()).collect::<Vec<_>>();
        let mut grouped_lines = merge_whitespace_only_lines(&raw_lines);
        rebalance_atomic_tails(&mut grouped_lines);

        if grouped_lines.len() == 1 {
            buffer = candidate;
            continue;
        }

        for line in &grouped_lines[..grouped_lines.len() - 1] {
            lines.push(render_line(line, false));
        }
        buffer.clone_from(
            grouped_lines
                .last()
                .expect("merged wrapped lines include a trailing line"),
        );
    }

    if !buffer.is_empty() {
        lines.push(render_line(&buffer, true));
    }

    lines
}
