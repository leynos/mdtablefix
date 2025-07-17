//! Text wrapping utilities respecting inline code and prefixes.
//!
//! Unicode width handling follows `docs/unicode-width.md` lines 1-9 using the
//! `unicode-width` crate for accurate display calculations.

use regex::Regex;

static FENCE_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(```|~~~).*").unwrap());

static BULLET_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\s*(?:[-*+]|\d+[.)])\s+)(.*)").unwrap());

static FOOTNOTE_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\s*)(\[\^[^]]+\]:\s*)(.*)$").unwrap());

static BLOCKQUOTE_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\s*(?:>\s*)+)(.*)$").unwrap());

pub(crate) fn tokenize_markdown(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            let start = i;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
        } else if c == '`' {
            let start = i;
            let mut delim_len = 0;
            while i < chars.len() && chars[i] == '`' {
                i += 1;
                delim_len += 1;
            }
            let mut end = i;
            while end < chars.len() {
                if chars[end] == '`' {
                    let mut j = end;
                    let mut count = 0;
                    while j < chars.len() && chars[j] == '`' {
                        j += 1;
                        count += 1;
                    }
                    if count == delim_len {
                        end = j;
                        break;
                    }
                }
                end += 1;
            }
            if end >= chars.len() {
                tokens.push(chars[start..start + delim_len].iter().collect());
                i = start + delim_len;
            } else {
                tokens.push(chars[start..end].iter().collect());
                i = end;
            }
        } else {
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '`' {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
        }
    }
    tokens
}

/// Determine if the current line should break at the last whitespace.
///
/// Returns `true` if `current_width` exceeds `width` and a whitespace split
/// position is available.
///
/// # Examples
///
/// ```
/// use mdtablefix::wrap::should_break_line;
/// assert!(should_break_line(10, 12, Some(3)));
/// assert!(!should_break_line(10, 8, Some(3)));
/// ```
fn should_break_line(width: usize, current_width: usize, last_split: Option<usize>) -> bool {
    current_width > width && last_split.is_some()
}

fn wrap_preserving_code(text: &str, width: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    let mut last_split: Option<usize> = None;
    for token in tokenize_markdown(text) {
        let token_width = UnicodeWidthStr::width(token.as_str());
        if current_width + token_width <= width {
            current.push_str(&token);
            current_width += token_width;
            if token.chars().all(char::is_whitespace) {
                last_split = Some(current.len());
            }
            continue;
        }

        if should_break_line(width, current_width + token_width, last_split) {
            let pos = last_split.unwrap();
            let line = current[..pos].to_string();
            let mut rest = current[pos..].trim_start().to_string();
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                lines.push(trimmed.to_string());
            }
            rest.push_str(&token);
            current = rest;
            current_width = UnicodeWidthStr::width(current.as_str());
            last_split = if token.chars().all(char::is_whitespace) {
                Some(current.len())
            } else {
                None
            };
            if current_width > width {
                lines.push(current.trim_end().to_string());
                current.clear();
                current_width = 0;
                last_split = None;
            }
            continue;
        }

        let trimmed = current.trim_end();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
        current.clear();
        current_width = 0;

        if !token.chars().all(char::is_whitespace) {
            current.push_str(&token);
            current_width = token_width;
        }
    }
    let trimmed = current.trim_end();
    if !trimmed.is_empty() {
        lines.push(trimmed.to_string());
    }
    lines
}

#[doc(hidden)]
pub fn is_fence(line: &str) -> bool { FENCE_RE.is_match(line) }

fn flush_paragraph(out: &mut Vec<String>, buf: &[(String, bool)], indent: &str, width: usize) {
    if buf.is_empty() {
        return;
    }
    let mut segment = String::new();
    for (text, hard_break) in buf {
        if !segment.is_empty() {
            segment.push(' ');
        }
        segment.push_str(text);
        if *hard_break {
            for line in wrap_preserving_code(&segment, width - indent.len()) {
                out.push(format!("{indent}{line}"));
            }
            segment.clear();
        }
    }
    if !segment.is_empty() {
        for line in wrap_preserving_code(&segment, width - indent.len()) {
            out.push(format!("{indent}{line}"));
        }
    }
}

fn append_wrapped_with_prefix(
    out: &mut Vec<String>,
    prefix: &str,
    text: &str,
    width: usize,
    repeat_prefix: bool,
) {
    use unicode_width::UnicodeWidthStr;

    let prefix_width = UnicodeWidthStr::width(prefix);
    let available = width.saturating_sub(prefix_width).max(1);
    let indent_str: String = prefix.chars().take_while(|c| c.is_whitespace()).collect();
    let indent_width = UnicodeWidthStr::width(indent_str.as_str());
    let wrapped_indent = if repeat_prefix {
        prefix.to_string()
    } else {
        format!("{}{}", indent_str, " ".repeat(prefix_width - indent_width))
    };

    let lines = wrap_preserving_code(text, available);
    if lines.is_empty() {
        out.push(prefix.to_string());
        return;
    }

    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            out.push(format!("{prefix}{line}"));
        } else {
            out.push(format!("{wrapped_indent}{line}"));
        }
    }
}

fn handle_prefix_line(
    out: &mut Vec<String>,
    buf: &mut Vec<(String, bool)>,
    indent: &mut String,
    width: usize,
    prefix: &str,
    rest: &str,
    repeat_prefix: bool,
) {
    flush_paragraph(out, buf, indent, width);
    buf.clear();
    indent.clear();
    append_wrapped_with_prefix(out, prefix, rest, width, repeat_prefix);
}

/// Wrap text lines to the given width.
///
/// # Panics
/// Panics if regex captures fail unexpectedly.
#[must_use]
pub fn wrap_text(lines: &[String], width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf: Vec<(String, bool)> = Vec::new();
    let mut indent = String::new();
    let mut in_code = false;

    for line in lines {
        if FENCE_RE.is_match(line) {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            in_code = !in_code;
            out.push(line.clone());
            continue;
        }

        if in_code {
            out.push(line.clone());
            continue;
        }

        if line.trim_start().starts_with('|') || crate::table::SEP_RE.is_match(line.trim()) {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            out.push(line.clone());
            continue;
        }

        if line.trim_start().starts_with('#') {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            out.push(line.clone());
            continue;
        }

        if line.trim().is_empty() {
            flush_paragraph(&mut out, &buf, &indent, width);
            buf.clear();
            indent.clear();
            out.push(String::new());
            continue;
        }

        if let Some(cap) = BULLET_RE.captures(line) {
            let prefix = cap.get(1).unwrap().as_str();
            let rest = cap.get(2).unwrap().as_str();
            handle_prefix_line(&mut out, &mut buf, &mut indent, width, prefix, rest, false);
            continue;
        }

        if let Some(cap) = FOOTNOTE_RE.captures(line) {
            let indent_part = cap.get(1).unwrap().as_str();
            let label_part = cap.get(2).unwrap().as_str();
            let prefix = format!("{indent_part}{label_part}");
            let rest = cap.get(3).unwrap().as_str();
            handle_prefix_line(&mut out, &mut buf, &mut indent, width, &prefix, rest, false);
            continue;
        }

        if let Some(cap) = BLOCKQUOTE_RE.captures(line) {
            let prefix = cap.get(1).unwrap().as_str();
            let rest = cap.get(2).unwrap().as_str();
            handle_prefix_line(&mut out, &mut buf, &mut indent, width, prefix, rest, true);
            continue;
        }

        if buf.is_empty() {
            indent = line.chars().take_while(|c| c.is_whitespace()).collect();
        }
        let trimmed_end = line.trim_end();
        let hard_break = line.ends_with("  ")
            || trimmed_end.ends_with("<br>")
            || trimmed_end.ends_with("<br/>")
            || trimmed_end.ends_with("<br />");
        let text = trimmed_end
            .trim_end_matches("<br>")
            .trim_end_matches("<br/>")
            .trim_end_matches("<br />")
            .trim_end_matches(' ')
            .trim_start()
            .to_string();
        buf.push((text, hard_break));
    }

    flush_paragraph(&mut out, &buf, &indent, width);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_text_preserves_hyphenated_words() {
        let input = vec!["A word that is very-long-word indeed".to_string()];
        let wrapped = wrap_text(&input, 20);
        assert_eq!(
            wrapped,
            vec![
                "A word that is".to_string(),
                "very-long-word".to_string(),
                "indeed".to_string(),
            ]
        );
    }

    #[test]
    fn wrap_text_does_not_insert_spaces_in_hyphenated_words() {
        let input = vec![
            concat!(
                "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Donec tincidunt ",
                "elit-sed fermentum congue. Vivamus dictum nulla sed consectetur ",
                "volutpat."
            )
            .to_string(),
        ];
        let wrapped = wrap_text(&input, 80);
        assert_eq!(
            wrapped,
            vec![
                "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Donec tincidunt"
                    .to_string(),
                "elit-sed fermentum congue. Vivamus dictum nulla sed consectetur volutpat."
                    .to_string(),
            ]
        );
    }

    #[test]
    fn wrap_text_preserves_code_spans() {
        let input = vec![
            "with their own escaping rules. On Windows, scripts default to `powershell -Command` \
             unless the manifest's `interpreter` field overrides the setting."
                .to_string(),
        ];
        let wrapped = wrap_text(&input, 60);
        assert_eq!(
            wrapped,
            vec![
                "with their own escaping rules. On Windows, scripts default".to_string(),
                "to `powershell -Command` unless the manifest's".to_string(),
                "`interpreter` field overrides the setting.".to_string(),
            ]
        );
    }

    #[test]
    fn wrap_text_multiple_code_spans() {
        let input = vec!["combine `foo bar` and `baz qux` in one line".to_string()];
        let wrapped = wrap_text(&input, 25);
        assert_eq!(
            wrapped,
            vec![
                "combine `foo bar` and".to_string(),
                "`baz qux` in one line".to_string(),
            ]
        );
    }

    #[test]
    fn wrap_text_nested_backticks() {
        let input = vec!["Use `` `code` `` to quote backticks".to_string()];
        let wrapped = wrap_text(&input, 20);
        assert_eq!(
            wrapped,
            vec![
                "Use `` `code` `` to".to_string(),
                "quote backticks".to_string()
            ]
        );
    }

    #[test]
    fn wrap_text_unmatched_backticks() {
        let input = vec!["This has a `dangling code span.".to_string()];
        let wrapped = wrap_text(&input, 20);
        assert_eq!(
            wrapped,
            vec!["This has a".to_string(), "`dangling code span.".to_string()]
        );
    }

    #[test]
    fn wrap_text_preserves_links() {
        let input = vec![
            "`falcon-pachinko` is an extension library for the".to_string(),
            "[Falcon](https://falcon.readthedocs.io) web framework. It adds a structured"
                .to_string(),
            "approach to asynchronous WebSocket routing and background worker integration."
                .to_string(),
        ];
        let wrapped = wrap_text(&input, 80);
        let joined = wrapped.join("\n");
        assert_eq!(joined.matches("https://").count(), 1);
        assert!(
            wrapped
                .iter()
                .any(|l| l.contains("https://falcon.readthedocs.io"))
        );
    }
}
