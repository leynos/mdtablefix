//! Inline footnote helpers.
//!
//! Handles inline reference detection and heading detection so the
//! top-level converter can focus on orchestration.

use std::sync::LazyLock;

use regex::{Captures, Regex};

/// Matches punctuation followed by a bare numeric reference in prose.
///
/// The punctuation and surrounding text are captured so replacement preserves
/// the source layout while changing only the reference marker.
static INLINE_FN_RE: LazyLock<Regex> = lazy_regex!(
    r"(?P<pre>^|[^0-9])(?P<punc>[.!?);:])(?P<style>[*_]*)(?P<num>\d+)(?P<boundary>\s|$)",
    "inline footnote reference pattern should compile",
);

/// Matches the alternate `number:` spelling used for inline references.
///
/// Its captures retain whitespace, emphasis, and extra colons because those
/// belong to the source syntax and must survive conversion.
static COLON_FN_RE: LazyLock<Regex> = lazy_regex!(
    r"(?P<pre>^|[^0-9])\s+(?P<style>[*_]*)(?P<num>\d+)\s*:(?P<colons>:*)(?P<boundary>\s|[[:punct:]]|$)",
    "space-colon footnote reference pattern should compile",
);

/// Recognises an ATX heading prefix before inline footnote conversion.
///
/// Heading lines are passed through so a heading's numeric text is not treated
/// as a prose reference.
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

/// Borrowed captures that reconstruct one prose footnote without moving text.
///
/// Each slice names a source boundary, so replacement can preserve punctuation
/// and emphasis exactly while changing only the numeric reference.
#[derive(Clone, Copy)]
struct InlineFootnote<'a> {
    /// Text preceding the punctuation or whitespace that introduced the number.
    pre: &'a str,
    /// Punctuation that separates prose from the reference number.
    punc: &'a str,
    /// Emphasis markers that belong immediately before the reference.
    style: &'a str,
    /// Digits that identify the source footnote.
    num: &'a str,
    /// Boundary text retained after the generated reference.
    boundary: &'a str,
}

/// Copies the named captures needed to rebuild an inline footnote reference.
///
/// Keeping these slices borrowed from the regex match avoids changing text
/// while the replacement callback is still deciding how to render it.
#[inline]
fn capture_parts<'a>(caps: &'a Captures<'a>) -> InlineFootnote<'a> {
    InlineFootnote {
        pre: &caps["pre"],
        punc: &caps["punc"],
        style: &caps["style"],
        num: &caps["num"],
        boundary: &caps["boundary"],
    }
}

/// Renders captured source pieces as a GFM footnote reference.
///
/// The original punctuation, emphasis, and boundary are retained so replacing
/// a number does not alter adjacent prose.
#[inline]
fn build_footnote(parts: InlineFootnote<'_>) -> String {
    format!(
        "{}{}{}[^{}]{}",
        parts.pre, parts.punc, parts.style, parts.num, parts.boundary
    )
}

/// Convert inline numeric references into Markdown footnote syntax.
pub(super) fn convert_inline(text: &str) -> String {
    let out = INLINE_FN_RE.replace_all(text, |caps: &Captures| build_footnote(capture_parts(caps)));
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
pub(super) fn is_atx_heading_prefix(s: &str) -> bool { ATX_HEADING_RE.is_match(s) }
