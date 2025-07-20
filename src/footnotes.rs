//! Footnote normalisation utilities.
//!
//! Converts bare numeric references in text to GitHub-flavoured Markdown
//! footnote links and rewrites the trailing numeric list into a footnote
//! block. Only the final contiguous list of footnotes is processed.

use regex::{Captures, Regex};

static INLINE_FN_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"(?P<pre>^|[^0-9])(?P<punc>[.!?);:])(?P<style>[*_]*)(?P<num>\d+)(?P<boundary>\s|$)")
        .unwrap()
});

static FOOTNOTE_LINE_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"^(?P<indent>\s*)(?P<num>\d+)\.\s+(?P<rest>.*)$").unwrap()
});

use crate::wrap::{Token, tokenize_markdown};

fn convert_inline(text: &str) -> String {
    INLINE_FN_RE
        .replace_all(text, |caps: &Captures| {
            format!(
                "{}{}{}[^{}]{}",
                &caps["pre"], &caps["punc"], &caps["style"], &caps["num"], &caps["boundary"]
            )
        })
        .into_owned()
}

fn convert_block(lines: &mut [String]) {
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |i| i + 1);
    let start = (0..end)
        .rfind(|&i| !FOOTNOTE_LINE_RE.is_match(lines[i].trim_end()))
        .map_or(0, |i| i + 1);

    if start >= end || lines[start].trim_start().starts_with("[^") {
        return;
    }

    for line in &mut lines[start..end] {
        *line = FOOTNOTE_LINE_RE
            .replace(line, "${indent}[^${num}] ${rest}")
            .to_string();
    }
}

/// Convert bare numeric footnote references to Markdown footnote syntax.
#[must_use]
pub fn convert_footnotes(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }
    let joined = lines.join("\n");
    let mut out = String::new();
    for token in tokenize_markdown(&joined) {
        match token {
            Token::Text(t) => out.push_str(&convert_inline(t)),
            Token::Code(c) => {
                out.push('`');
                out.push_str(c);
                out.push('`');
            }
            Token::Fence(f) => out.push_str(f),
            Token::Newline => out.push('\n'),
        }
    }
    let mut lines: Vec<String> = out.split('\n').map(str::to_string).collect();
    convert_block(&mut lines);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_inline_numbers() {
        let input = vec!["See the docs.2".to_string()];
        let expected = vec!["See the docs.[^2]".to_string()];
        assert_eq!(convert_footnotes(&input), expected);
    }

    #[test]
    fn converts_final_list() {
        let input = vec![
            "Text.".to_string(),
            String::new(),
            " 1. First".to_string(),
            " 2. Second".to_string(),
        ];
        let expected = vec![
            "Text.".to_string(),
            String::new(),
            " [^1] First".to_string(),
            " [^2] Second".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), expected);
    }

    #[test]
    fn idempotent_on_existing_block() {
        let input = vec![" [^1] First".to_string()];
        assert_eq!(convert_footnotes(&input), input);
    }
}
