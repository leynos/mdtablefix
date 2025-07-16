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
pub fn format_breaks(lines: &[String]) -> Vec<std::borrow::Cow<'_, str>> {
    use std::borrow::Cow;

    let mut out = Vec::with_capacity(lines.len());
    let mut in_code = false;

    for line in lines {
        if is_fence(line) {
            in_code = !in_code;
            out.push(Cow::Borrowed(line.as_str()));
            continue;
        }

        if !in_code && THEMATIC_BREAK_RE.is_match(line.trim_end()) {
            out.push(Cow::Owned(THEMATIC_BREAK_LINE.clone()));
        } else {
            out.push(Cow::Borrowed(line.as_str()));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_formatting() {
        let input = vec!["foo".to_string(), "***".to_string(), "bar".to_string()];
        let expected = vec![
            "foo".to_string(),
            "_".repeat(THEMATIC_BREAK_LEN),
            "bar".to_string(),
        ];
        let result: Vec<String> =
            format_breaks(&input).into_iter().map(std::borrow::Cow::into_owned).collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn ignores_fenced_code() {
        let input = vec!["```".to_string(), "---".to_string(), "```".to_string()];
        let result: Vec<String> =
            format_breaks(&input).into_iter().map(std::borrow::Cow::into_owned).collect();
        assert_eq!(result, input);
    }
}
