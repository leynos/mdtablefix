//! Heading normalisation helpers.
//!
//! This module converts Setext-style headings (underlined with sequences of three or
//! more `=` or `-` characters) into ATX headings that use leading hash markers.
//! Normalising the heading style allows downstream processing such as wrapping to
//! treat the headings consistently.

use crate::wrap::FenceTracker;

/// Convert Setext-style headings into ATX (`#`) headings.
///
/// Lines that are part of fenced code blocks are left unchanged. The function preserves
/// leading blockquote markers and indentation shared by the heading and its underline.
#[must_use]
pub fn convert_setext_headings(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut fence_tracker = FenceTracker::default();
    let mut idx = 0;

    while idx < lines.len() {
        let line = &lines[idx];

        if fence_tracker.observe(line) {
            out.push(line.clone());
            idx += 1;
            continue;
        }

        if fence_tracker.in_fence() {
            out.push(line.clone());
            idx += 1;
            continue;
        }

        if let Some((level, prefix_len, text)) = detect_setext_heading(line, lines.get(idx + 1)) {
            let prefix = &line[..prefix_len];
            out.push(build_heading_line(prefix, level, &text));
            idx += 2;
            continue;
        }

        out.push(line.clone());
        idx += 1;
    }

    out
}

fn detect_setext_heading(line: &str, underline: Option<&String>) -> Option<(usize, usize, String)> {
    let underline = underline?;
    if line.trim().is_empty() {
        return None;
    }

    let prefix_len = shared_prefix_len(line, underline);
    if prefix_len > 0
        && !line[..prefix_len]
            .chars()
            .all(|c| c.is_whitespace() || c == '>')
    {
        return None;
    }
    let text = line[prefix_len..].trim();
    if text.is_empty() {
        return None;
    }

    let underline_body = underline[prefix_len..].trim();
    if underline_body.is_empty() {
        return None;
    }

    let marker = underline_body.chars().next()?;
    if marker != '=' && marker != '-' {
        return None;
    }
    if !underline_body.chars().all(|c| c == marker) {
        return None;
    }
    if underline_body.chars().count() < 3 {
        return None;
    }

    let level = if marker == '=' { 1 } else { 2 };
    Some((level, prefix_len, text.to_string()))
}

fn shared_prefix_len(a: &str, b: &str) -> usize {
    let mut end = 0;
    let mut iter_a = a.char_indices();
    let mut iter_b = b.char_indices();

    loop {
        match (iter_a.next(), iter_b.next()) {
            (Some((idx_a, ch_a)), Some((_, ch_b))) if ch_a == ch_b => {
                end = idx_a + ch_a.len_utf8();
            }
            _ => break,
        }
    }

    end
}

fn build_heading_line(prefix: &str, level: usize, text: &str) -> String {
    let mut heading = String::new();
    heading.push_str(prefix);
    if !prefix.is_empty() && !prefix.chars().last().is_some_and(char::is_whitespace) {
        heading.push(' ');
    }
    heading.push_str(&"#".repeat(level));
    if !text.is_empty() {
        heading.push(' ');
        heading.push_str(text);
    }
    heading
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(vec!["Heading".into(), "===".into()], vec!["# Heading".into()])]
    #[case(vec!["Heading".into(), "----".into()], vec!["## Heading".into()])]
    #[case(vec!["Title   ".into(), "=====".into()], vec!["# Title".into()])]
    #[case(vec!["   Heading".into(), "   ====".into()], vec!["   # Heading".into()])]
    #[case(
        vec!["> Quote".into(), "> ----".into()],
        vec!["> ## Quote".into()]
    )]
    fn converts_setext_headings(#[case] input: Vec<String>, #[case] expected: Vec<String>) {
        assert_eq!(convert_setext_headings(&input), expected);
    }

    #[rstest]
    #[case(vec!["```".into(), "Heading".into(), "---".into(), "```".into()])]
    #[case(vec!["Not a heading".into(), "--".into()])]
    #[case(vec!["- Item".into(), "-----".into()])]
    #[case(vec![String::new(), "---".into()])]
    fn leaves_non_headings_untouched(#[case] lines: Vec<String>) {
        assert_eq!(convert_setext_headings(&lines), lines);
    }
}
