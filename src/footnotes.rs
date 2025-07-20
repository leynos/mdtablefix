//! Footnote normalisation utilities.
//!
//! Converts bare numeric references in text to GitHub-flavoured Markdown
//! footnote links and rewrites the trailing numeric list into a footnote
//! block.

use regex::Regex;

use crate::wrap::{Token, tokenize_markdown};

static FOOTNOTE_LINE_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"^(?P<indent>\s*)(?P<num>\d+)\.\s+(?P<rest>.*)$").unwrap()
});

fn convert_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if matches!(ch, '.' | '!' | '?' | ')' | ';' | ':')
            && (i == 0 || !chars[i - 1].is_ascii_digit())
        {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && (j == chars.len() || chars[j].is_whitespace()) {
                out.push(ch);
                out.push_str("[^");
                for c in &chars[i + 1..j] {
                    out.push(*c);
                }
                out.push(']');
                if j < chars.len() {
                    out.push(chars[j]);
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn convert_block(lines: &mut [String]) {
    let mut end = lines.len();
    while end > 0 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 {
        if FOOTNOTE_LINE_RE.is_match(lines[start - 1].trim_end()) {
            start -= 1;
        } else {
            break;
        }
    }
    if start >= end {
        return;
    }
    if lines[start].trim_start().starts_with("[^") {
        return;
    }
    for line in lines.iter_mut().take(end).skip(start) {
        if let Some(cap) = FOOTNOTE_LINE_RE.captures(line.as_str()) {
            let indent = cap.name("indent").unwrap().as_str();
            let num = cap.name("num").unwrap().as_str();
            let rest = cap.name("rest").unwrap().as_str();
            *line = format!("{indent}[^{num}] {rest}");
        }
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
