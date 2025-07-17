//! Ordered list renumbering utilities.

use regex::Regex;

use crate::wrap::is_fence;

fn parse_numbered(line: &str) -> Option<(&str, &str, &str)> {
    static NUMBERED_RE: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"^(\s*)([1-9][0-9]*)\.(\s+)(.*)").unwrap());
    let cap = NUMBERED_RE.captures(line)?;
    let indent = cap.get(1)?.as_str();
    let sep = cap.get(3)?.as_str();
    let rest = cap.get(4)?.as_str();
    Some((indent, sep, rest))
}

fn indent_len(indent: &str) -> usize {
    indent
        .chars()
        .fold(0, |acc, ch| acc + if ch == '\t' { 4 } else { 1 })
}

fn drop_deeper(indent: usize, counters: &mut Vec<(usize, usize)>) {
    while counters.last().is_some_and(|(d, _)| *d > indent) {
        counters.pop();
    }
}

fn is_plain_paragraph_line(line: &str) -> bool {
    line.trim_start()
        .chars()
        .next()
        .is_some_and(char::is_alphanumeric)
}

/// Remove counters deeper than or equal to `indent`.
///
/// ```
/// use mdtablefix::lists::pop_counters_upto;
/// let mut counters = vec![(0usize, 1usize), (4, 2), (8, 3)];
/// pop_counters_upto(&mut counters, 4);
/// assert_eq!(counters, vec![(0, 1)]);
/// ```
pub fn pop_counters_upto(counters: &mut Vec<(usize, usize)>, indent: usize) {
    while counters.last().is_some_and(|(d, _)| *d >= indent) {
        counters.pop();
    }
}

fn handle_paragraph_restart(line: &str, prev_blank: bool, counters: &mut Vec<(usize, usize)>) {
    let indent_end = line
        .char_indices()
        .find(|&(_, c)| !c.is_whitespace())
        .map_or_else(|| line.len(), |(i, _)| i);
    let indent = indent_len(&line[..indent_end]);

    if prev_blank
        && counters
            .last()
            .is_some_and(|(d, _)| indent <= *d && is_plain_paragraph_line(line))
    {
        pop_counters_upto(counters, indent);
    }
}

#[must_use]
pub fn renumber_lists(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut counters: Vec<(usize, usize)> = Vec::new();
    let mut in_code = false;
    let mut prev_blank = lines.first().is_none_or(|l| l.trim().is_empty());

    for line in lines {
        if is_fence(line) {
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

        if let Some((indent_str, sep, rest)) = parse_numbered(line) {
            let indent = indent_len(indent_str);
            drop_deeper(indent, &mut counters);
            let current = match counters.last_mut() {
                Some((d, cnt)) if *d == indent => {
                    *cnt += 1;
                    *cnt
                }
                _ => {
                    counters.push((indent, 1));
                    1
                }
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
        handle_paragraph_restart(line, prev_blank, &mut counters);
        drop_deeper(indent, &mut counters);
        out.push(line.clone());
        prev_blank = line.trim().is_empty();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
