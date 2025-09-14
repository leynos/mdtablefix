//! Ordered list renumbering utilities.

use regex::Regex;

use crate::{breaks::THEMATIC_BREAK_RE, wrap::is_fence};

/// Characters that mark formatted text at the start of a line.
const FORMATTING_CHARS: [char; 3] = ['*', '_', '`'];

// Lines starting with optional indentation followed by '#' characters denote
// Markdown ATX headings. A space or end of line must follow the hashes.
static HEADING_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"^[ ]{0,3}#{1,6}(?:\s|$)").expect("valid heading regex")
});

fn parse_numbered(line: &str) -> Option<(usize, &str, &str, &str)> {
    static NUMBERED_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"^(\s*)(?:[1-9][0-9]*)\.(\s+)(.*)").expect("valid list number regex")
    });
    let cap = NUMBERED_RE.captures(line)?;
    let indent_str = cap.get(1)?.as_str();
    let indent = indent_len(indent_str);
    let sep = cap.get(2)?.as_str();
    let rest = cap.get(3)?.as_str();
    Some((indent, indent_str, sep, rest))
}

/// Remove counters for indents deeper than the given level.
/// When `inclusive` is true, levels equal to `indent` are also removed.
fn prune_deeper(indent: usize, inclusive: bool, counters: &mut Vec<(usize, usize)>) {
    while match counters.last() {
        Some((d, _)) => {
            if inclusive {
                *d >= indent
            } else {
                *d > indent
            }
        }
        None => false,
    } {
        counters.pop();
    }
}

fn indent_len(indent: &str) -> usize {
    indent
        .chars()
        .fold(0, |acc, ch| acc + if ch == '\t' { 4 } else { 1 })
}

fn is_plain_paragraph_line(line: &str) -> bool {
    matches!(
        line.trim_start()
            .trim_start_matches(|c: char| FORMATTING_CHARS.contains(&c))
            .chars()
            .next(),
        Some(c) if c.is_alphanumeric()
    )
}

fn handle_paragraph_restart(
    indent: usize,
    line: &str,
    prev_blank: bool,
    counters: &mut Vec<(usize, usize)>,
) -> bool {
    let inclusive = if prev_blank {
        match counters.last() {
            Some((d, _)) => indent <= *d && is_plain_paragraph_line(line),
            None => false,
        }
    } else {
        false
    };
    if inclusive {
        prune_deeper(indent, true, counters);
    }
    inclusive
}

/// Renumber ordered Markdown list items.
///
/// Preserves code fences, resets numbering on headings and thematic breaks,
/// and restarts after a blank line followed by a plain paragraph at the same
/// or a shallower indent.
///
/// # Examples
///
/// ```
/// use mdtablefix::renumber_lists;
///
/// let lines = vec![
///     String::from("1. first"),
///     String::from("4. second"),
/// ];
/// assert_eq!(
///     renumber_lists(&lines),
///     vec![String::from("1. first"), String::from("2. second")]
/// );
/// ```
#[must_use]
pub fn renumber_lists(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut counters: Vec<(usize, usize)> = Vec::new();
    let mut in_code = false;
    let mut prev_blank = match lines.first() {
        Some(l) => l.trim().is_empty(),
        None => true,
    };

    for line in lines {
        if is_fence(line).is_some() {
            in_code = !in_code;
            out.push(line.clone());
            prev_blank = false;
            continue;
        }
        if in_code {
            out.push(line.clone());
            prev_blank = line.trim().is_empty();
            continue;
        }
        if line.trim().is_empty() {
            out.push(line.clone());
            prev_blank = true;
            continue;
        }
        if let Some((indent, indent_str, sep, rest)) = parse_numbered(line) {
            prune_deeper(indent, false, &mut counters);
            let current = if let Some((d, cnt)) = counters.last_mut() {
                if *d == indent {
                    *cnt += 1;
                    *cnt
                } else {
                    counters.push((indent, 1));
                    1
                }
            } else {
                counters.push((indent, 1));
                1
            };
            out.push(format!("{indent_str}{current}.{sep}{rest}"));
            prev_blank = false;
            continue;
        }
        let indent_end = line
            .char_indices()
            .find(|&(_, c)| !c.is_whitespace())
            .map_or_else(|| line.len(), |(i, _)| i);
        let indent_str = &line[..indent_end];
        let indent = indent_len(indent_str);
        if HEADING_RE.is_match(line) || THEMATIC_BREAK_RE.is_match(line.trim_end()) {
            counters.clear();
            out.push(line.clone());
            prev_blank = false;
            continue;
        }
        let did_inclusive = handle_paragraph_restart(indent, line, prev_blank, &mut counters);
        if !did_inclusive {
            prune_deeper(indent, false, &mut counters);
        }
        out.push(line.clone());
        prev_blank = false;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_numbered_parts() {
        let line = "  12. item";
        assert_eq!(parse_numbered(line), Some((2, "  ", " ", "item")));
    }

    #[test]
    fn parse_numbered_with_tab() {
        let line = "	1.	foo";
        assert_eq!(parse_numbered(line), Some((4, "	", "	", "foo")));
    }

    #[test]
    fn simple_renumber() {
        let input = vec!["1. a", "3. b"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let expected = vec!["1. a", "2. b"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(renumber_lists(&input), expected);
    }

    #[test]
    fn nested_renumber() {
        let input = vec!["1. a", "    1. sub", "    3. sub2", "2. b"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let expected = vec!["1. a", "    1. sub", "    2. sub2", "2. b"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(renumber_lists(&input), expected);
    }
}
