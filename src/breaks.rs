//! Thematic break formatting utilities.

use regex::Regex;

use crate::wrap::is_fence;

pub const THEMATIC_BREAK_LEN: usize = 70;

static THEMATIC_BREAK_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"^[ ]{0,3}((?:[ \t]*\*){3,}|(?:[ \t]*-){3,}|(?:[ \t]*_){3,})[ \t]*$").unwrap()
});

static THEMATIC_BREAK_LINE: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| "_".repeat(THEMATIC_BREAK_LEN));

#[must_use]
pub fn format_breaks(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut in_code = false;

    for line in lines {
        if is_fence(line) {
            in_code = !in_code;
            out.push(line.clone());
            continue;
        }

        if !in_code && THEMATIC_BREAK_RE.is_match(line.trim_end()) {
            out.push(THEMATIC_BREAK_LINE.clone());
        } else {
            out.push(line.clone());
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_formatting() {
        let input = vec!["foo", "***", "bar"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let expected = vec![
            "foo".to_string(),
            "_".repeat(THEMATIC_BREAK_LEN),
            "bar".to_string(),
        ];
        assert_eq!(format_breaks(&input), expected);
    }

    #[test]
    fn ignores_fenced_code() {
        let input = vec!["```", "---", "```"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(format_breaks(&input), input);
    }
}
