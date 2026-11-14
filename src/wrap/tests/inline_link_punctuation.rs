//! Punctuation coupling cases for links under wrapping.
//!
//! A quoted link such as `"[Link](foo.md)!"` carries punctuation on both sides,
//! and the wrapper must keep both with the link instead of stranding a lone
//! quote at a line boundary. The trailing cases add the asymmetric form, where
//! only the closing mark is attached and the link must not be split from it.
//!
//! These cases live beside `inline_wrapping` rather than inside it to keep that
//! module under the file-size ceiling.

use rstest::rstest;

use super::super::wiring::wrap_preserving_code;

#[test]
fn wrap_preserving_code_handles_leading_link_punctuation() {
    let input = concat!(
        "\"[Quoted link](quote.md)\" is important for understanding the ",
        "overall design because it provides context to the guidelines."
    );
    let lines = wrap_preserving_code(input, 80);
    assert!(lines.len() > 1, "expected wrapping for {input:?}");
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("\"[Quoted link](quote.md)")),
        "expected leading punctuation to stay with link in {lines:?}",
    );
    assert!(
        lines.iter().all(|line| line.trim() != "\""),
        "leading punctuation was orphaned in {lines:?}",
    );
}

#[test]
fn wrap_preserving_code_handles_leading_and_trailing_link_punctuation() {
    let input = concat!(
        "\"[Link](foo.md)!\" demonstrates punctuation around a link and ",
        "includes plenty of extra words to exceed the wrapping limit."
    );
    let lines = wrap_preserving_code(input, 80);
    assert!(lines.len() > 1, "expected wrapping for {input:?}");
    assert!(
        lines.iter().any(|line| line.contains("[Link](foo.md)!\"")),
        "expected trailing punctuation to stay with link in {lines:?}",
    );
    assert!(
        lines.iter().all(|line| line.trim() != "\""),
        "leading punctuation was orphaned in {lines:?}",
    );
}

#[rstest]
#[case("Check this [link](foo.md)!?", 11, "[link](foo.md)!?", "!?")]
#[case("Reference [doc](bar.md):", 9, "[doc](bar.md):", ":")]
#[case("See [note](baz.md)...", 3, "[note](baz.md)...", "...")]
#[case("Alert [warn](warn.md);", 5, "[warn](warn.md);", ";")]
fn wrap_preserving_code_keeps_trailing_link_punctuation(
    #[case] input: &str,
    #[case] width: usize,
    #[case] expected_link: &str,
    #[case] orphaned_punctuation: &str,
) {
    let lines = wrap_preserving_code(input, width);
    assert!(lines.len() > 1, "expected wrapping for {input:?}");
    assert!(
        lines.iter().any(|line| line.contains(expected_link)),
        "expected {expected_link:?} to stay attached in {lines:?}",
    );
    assert!(
        lines.iter().all(|line| line.trim() != orphaned_punctuation),
        "punctuation was orphaned in {lines:?}",
    );
}
