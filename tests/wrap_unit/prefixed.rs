//! `wrap_text` tests covering trailing-space hard breaks, headings, and
//! indented passthrough.

use mdtablefix::wrap::wrap_text;
use rstest::rstest;

#[rstest]
#[case(lines_vec!["> word1 word2  "], lines_vec!["> word1", "> word2  "])]
#[case(lines_vec!["- word1 word2  "], lines_vec!["- word1", "  word2  "])]
fn wrap_text_keeps_trailing_spaces(#[case] input: Vec<String>, #[case] expected: Vec<String>) {
    let wrapped = wrap_text(&input, 10);
    assert_eq!(wrapped, expected);
}

#[test]
fn wrap_text_preserves_indented_hash_as_text() {
    let input = lines_vec!["Paragraph intro.", "    # code", "Continuation."];
    let wrapped = wrap_text(&input, 40);
    assert_eq!(input, wrapped);
}

#[test]
fn wrap_text_flushes_before_heading() {
    let input = lines_vec!["Paragraph intro.", "# Heading", "Continuation."];
    let wrapped = wrap_text(&input, 40);
    assert_eq!(input, wrapped);
}
