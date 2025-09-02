//! Footnote normalisation utilities.
//!
//! Converts bare numeric references in text to GitHub-flavoured Markdown
//! footnote links and rewrites the trailing numeric list into a footnote
//! block. Only the final contiguous list of footnotes is processed.

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

static FOOTNOTE_DEF_RE: LazyLock<Regex> = lazy_regex!(
    r"^\s*(?:>\s*)*\[\^\d+\]:",
    "footnote definition pattern should compile",
);

use crate::textproc::{Token, process_tokens, push_original_token};

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
/// only definitions like `[^1]: note` cause skipping.
///
/// # Examples
///
/// ```ignore
/// let lines = vec!["[^1]: Old".to_string(), " 2. New".to_string()];
/// assert!(has_existing_footnote_block(&lines, 1));
/// ```
fn has_existing_footnote_block(lines: &[String], start: usize) -> bool {
    lines[..start].iter().any(|l| FOOTNOTE_DEF_RE.is_match(l))
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

/// Convert bare numeric footnote references to Markdown footnote syntax.
#[must_use]
pub fn convert_footnotes(lines: &[String]) -> Vec<String> {
    let mut lines = process_tokens(lines, |tok, out| match tok {
        Token::Text(t) => out.push_str(&convert_inline(t)),
        _ => push_original_token(&tok, out),
    });
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
            "[^10]: Tenth".to_string(),
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
            "[^2]: final".to_string(),
        ];
        assert_eq!(convert_footnotes(&input), expected);
    }
}
