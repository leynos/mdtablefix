//! `wrap_text` tests covering trailing-space hard breaks, headings, and
//! indented passthrough.

use mdtablefix::wrap::wrap_text;

#[test]
fn wrap_text_keeps_trailing_spaces_for_blockquote_final_line() {
    // "> " is the prefix; available width = 10 - 2 = 8.
    let input = lines_vec!["> word1 word2  "];
    let wrapped = wrap_text(&input, 10);
    assert_eq!(wrapped, lines_vec!["> word1", "> word2  "]);
}

#[test]
fn wrap_text_keeps_trailing_spaces_for_bullet_final_line() {
    // "- " is the prefix; continuation lines are indented with two spaces.
    let input = lines_vec!["- word1 word2  "];
    let wrapped = wrap_text(&input, 10);
    assert_eq!(wrapped, lines_vec!["- word1", "  word2  "]);
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
