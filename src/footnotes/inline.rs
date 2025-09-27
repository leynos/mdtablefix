//! Inline footnote helpers.
//!
//! Handles inline reference detection and heading detection so the
//! top-level converter can focus on orchestration.

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

static ATX_HEADING_RE: LazyLock<Regex> = lazy_regex!(
    r"(?x)
        ^\s*
        (?:>+\s*)*
        (?:[-*+]\s+|\d+[.)]\s+)*
        \#{1,6}
        (?:\s|$)
    ",
    "atx heading prefix",
);

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

#[inline]
fn build_footnote(pre: &str, punc: &str, style: &str, num: &str, boundary: &str) -> String {
    format!("{pre}{punc}{style}[^{num}]{boundary}")
}

/// Convert inline numeric references into Markdown footnote syntax.
pub(super) fn convert_inline(text: &str) -> String {
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
            let mat = caps.get(0).expect("regex matched without capture");
            let match_str = mat.as_str();
            let num_match = caps.name("num").expect("regex matched without num capture");
            let style_start = caps
                .name("style")
                .map_or(num_match.start() - mat.start(), |m| m.start() - mat.start());
            let captured_gap = &match_str[pre.len()..style_start];
            let gap = if pre.is_empty() {
                captured_gap
            } else if pre.chars().last().is_some_and(char::is_alphanumeric) {
                ""
            } else {
                captured_gap
            };
            format!("{pre}{gap}{style}[^{num}]:{colons}{boundary}")
        })
        .into_owned()
}

/// Determine whether a string is the prefix of an ATX heading.
pub(super) fn is_atx_heading_prefix(s: &str) -> bool {
    ATX_HEADING_RE.is_match(s)
}
