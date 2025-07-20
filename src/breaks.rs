//! Thematic break formatting utilities.

use std::borrow::Cow;

use regex::Regex;

use crate::wrap::is_fence;

pub const THEMATIC_BREAK_LEN: usize = 70;

pub(crate) static THEMATIC_BREAK_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"^[ ]{0,3}((?:[ \t]*\*){3,}|(?:[ \t]*-){3,}|(?:[ \t]*_){3,})[ \t]*$")
        .expect("valid thematic break regex")
});

static THEMATIC_BREAK_LINE: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| "_".repeat(THEMATIC_BREAK_LEN));

#[must_use]
pub fn format_breaks(lines: &[String]) -> Vec<Cow<'_, str>> {
    let mut out = Vec::with_capacity(lines.len());
    let mut in_code = false;

    for line in lines {
        if is_fence(line) {
            in_code = !in_code;
            out.push(Cow::Borrowed(line.as_str()));
            continue;
        }

        if !in_code && THEMATIC_BREAK_RE.is_match(line.trim_end()) {
            out.push(Cow::Borrowed(THEMATIC_BREAK_LINE.as_str()));
        } else {
            out.push(Cow::Borrowed(line.as_str()));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    #[test]
    fn basic_formatting() {
        let input = vec!["foo", "***", "bar"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let expected: Vec<Cow<str>> = vec![
            input[0].as_str().into(),
            Cow::Borrowed(THEMATIC_BREAK_LINE.as_str()),
            input[2].as_str().into(),
        ];
        assert_eq!(format_breaks(&input), expected);
    }

    #[test]
    fn ignores_fenced_code() {
        let input = vec!["```", "---", "```"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let expected: Vec<Cow<str>> = input.iter().map(|s| s.as_str().into()).collect();
        assert_eq!(format_breaks(&input), expected);
    }
}
