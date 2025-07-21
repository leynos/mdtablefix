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
    INLINE_FN_RE
        .replace_all(text, |caps: &Captures| {
            let (pre, punc, style, num, boundary) = capture_parts(caps);
            build_footnote(pre, punc, style, num, boundary)
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

fn convert_block(lines: &mut [String]) {
    let (footnote_start, trimmed_end) = trimmed_range(lines, |l| FOOTNOTE_LINE_RE.is_match(l));

    if footnote_start >= trimmed_end || lines[footnote_start].trim_start().starts_with("[^") {
        return;
    }

    for line in &mut lines[footnote_start..trimmed_end] {
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

    #[test]
    fn converts_block_after_existing_line() {
        let input = vec!["[^1] Old".to_string(), " 2. New".to_string()];
        let expected = vec!["[^1] Old".to_string(), " [^2] New".to_string()];
        assert_eq!(convert_footnotes(&input), expected);
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
            "2. final".to_string(),
        ];
        let expected = vec![
            "Intro.".to_string(),
            "1. not a footnote".to_string(),
            "More text.".to_string(),
            "[^2] final".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), expected);
    }
}
