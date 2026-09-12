//! Unit tests for heading conversion.

use rstest::rstest;

use super::*;

#[rstest]
#[case(vec!["Heading".into(), "===".into()], vec!["# Heading".into()])]
#[case(vec!["Heading".into(), "----".into()], vec!["## Heading".into()])]
#[case(vec!["Title   ".into(), "=====".into()], vec!["# Title".into()])]
#[case(vec!["   Heading".into(), "   ====".into()], vec!["   # Heading".into()])]
#[case(vec!["Heading".into(), "----   ".into()], vec!["## Heading".into()])]
#[case(
    vec!["> Quote".into(), "> ----".into()],
    vec!["> ## Quote".into()]
)]
#[case(
    vec![">> Title".into(), ">> ----".into()],
    vec![">> ## Title".into()]
)]
#[case(
    vec!["   > Title".into(), "   > -----".into()],
    vec!["   > ## Title".into()]
)]
fn converts_setext_headings(#[case] input: Vec<String>, #[case] expected: Vec<String>) {
    assert_eq!(convert_setext_headings(&input), expected);
}

#[rstest]
#[case(vec!["```".into(), "Heading".into(), "---".into(), "```".into()])]
#[case(vec!["Not a heading".into(), "--".into()])]
#[case(vec!["- Item".into(), "-----".into()])]
#[case(vec![String::new(), "---".into()])]
#[case(vec!["> Quote".into(), "-----".into()])]
#[case(vec![" Heading".into(), "  ---".into()])]
#[case(vec!["Heading".into(), "-==".into()])]
fn leaves_non_headings_untouched(#[case] lines: Vec<String>) {
    assert_eq!(convert_setext_headings(&lines), lines);
}

/// Asserts a candidate that is itself a block start keeps its underline.
///
/// Every case is a line a Setext underline followed, which the conversion
/// must refuse so the second line survives as a block of its own. The
/// `## aa` case is the reported reproduction: it became `## ## aa`, and the
/// thematic break below it disappeared.
#[rstest]
// An ATX heading at every level, with and without closing hashes.
#[case(vec!["# aa".into(), "===".into()])]
#[case(vec!["## aa".into(), "---".into()])]
#[case(vec!["### aa".into(), "===".into()])]
#[case(vec!["#### aa".into(), "---".into()])]
#[case(vec!["##### aa".into(), "===".into()])]
#[case(vec!["###### aa".into(), "---".into()])]
#[case(vec!["# aa #".into(), "===".into()])]
#[case(vec!["## aa ##".into(), "---".into()])]
#[case(vec!["###### aa ######".into(), "---".into()])]
// The same shapes behind a blockquote prefix, which the predicate sees
// only after the shared prefix has been removed.
#[case(vec!["> ## aa".into(), "> ---".into()])]
#[case(vec![">> # aa".into(), ">> ===".into()])]
#[case(vec!["   ### aa".into(), "   ---".into()])]
// Thematic breaks are block starts, not paragraph text.
#[case(vec!["---".into(), "---".into()])]
#[case(vec!["***".into(), "---".into()])]
#[case(vec!["___".into(), "---".into()])]
#[case(vec!["- - -".into(), "---".into()])]
// Table rows are table syntax, not paragraph text, so the break below one is
// not its underline. The repaired rows are what the table pass emits, and the
// quoted spelling reaches the predicate only after the shared prefix has been
// removed.
#[case(vec!["| --- | --- |".into(), "---".into()])]
#[case(vec!["|---|---|".into(), "---".into()])]
#[case(vec!["> | --- | --- |".into(), "> ---".into()])]
#[case(vec![">> |:--|--:|".into(), ">> ---".into()])]
#[case(vec!["   | --- | --- |".into(), "   ---".into()])]
#[case(vec!["| ccccc | d |".into(), "---".into()])]
#[case(vec!["| ccccc | d |".into(), "===".into()])]
#[case(vec!["> | ccccc | d |".into(), "> ---".into()])]
// List items, including the indented forms whose prefix is shared.
#[case(vec!["* item".into(), "-----".into()])]
#[case(vec!["  - item".into(), "  ---".into()])]
#[case(vec!["1. item".into(), "---".into()])]
#[case(vec!["1) item".into(), "---".into()])]
#[case(vec!["- [x] task".into(), "---".into()])]
// Definitions and directives.
#[case(vec!["[^1]: note".into(), "---".into()])]
#[case(vec!["[label]: https://example.com".into(), "---".into()])]
#[case(vec!["<!-- markdownlint-disable MD013 -->".into(), "---".into()])]
// Four columns of indentation make both lines an indented code block.
#[case(vec!["    code".into(), "    ---".into()])]
#[case(vec!["    code".into(), "    ===".into()])]
#[case(vec!["\tcode".into(), "\t---".into()])]
#[case(vec![">     code".into(), ">     ---".into()])]
#[case(vec![">>     code".into(), ">>     ===".into()])]
// The same pairs behind one, two, and three spaces before the marker, which
// CommonMark still reads as a blockquote holding indented code.
#[case(vec![" >     code".into(), " >     ---".into()])]
#[case(vec!["  >     code".into(), "  >     ---".into()])]
#[case(vec!["   >     code".into(), "   >     ---".into()])]
fn refuses_underlines_below_a_block_start(#[case] lines: Vec<String>) {
    assert_eq!(convert_setext_headings(&lines), lines);
}

/// Asserts the indentation width is measured inside any blockquote markers.
///
/// Three columns or fewer stay paragraph text and still convert; four or
/// more are an indented code block. The single space after each `>` marker
/// belongs to the marker, not to the content.
#[rstest]
#[case("code", 0)]
#[case("   code", 3)]
#[case("    code", 4)]
#[case("\tcode", 4)]
#[case("> code", 0)]
#[case(">   code", 2)]
#[case(">     code", 4)]
#[case(" > code", 0)]
#[case(" >     code", 4)]
#[case("  >     code", 4)]
#[case("   >     code", 4)]
// Four leading spaces are indented code, so no marker is consumed.
#[case("    >     code", 4)]
#[case(">> # aa", 0)]
#[case(">>     code", 4)]
fn measures_content_indentation(#[case] line: &str, #[case] expected: usize) {
    assert_eq!(content_indent_width(line), expected);
}

/// Asserts the predicate rejects block starts and admits paragraph text.
///
/// The payload table covers classes the line-pair tests cannot reach: a
/// candidate whose payload keeps a blockquote marker is refused earlier, by
/// the prefix match, and a fence marker line is skipped by the fence
/// tracker before detection.
#[rstest]
#[case("## aa", false)]
#[case("# aa #", false)]
#[case("---", false)]
#[case("***", false)]
#[case("- item", false)]
#[case("1. item", false)]
#[case("> quote", false)]
#[case("[^1]: note", false)]
#[case("[label]: https://example.com", false)]
#[case("<!-- markdownlint-disable MD013 -->", false)]
#[case("```", false)]
#[case("~~~", false)]
// Table rows are table syntax, not paragraph text. Each one is a candidate the
// table pass has just laid out, and the line below it in the reported shape is
// a thematic break rather than an underline. The delimiter rows come first, then
// the body and header rows the same rule was widened to cover.
#[case("| --- | --- |", false)]
#[case("|---|---|", false)]
#[case("--- | ---", false)]
#[case("|:--|--:|", false)]
#[case("| a | b |", false)]
#[case("| a | b", false)]
#[case("|  |  |", false)]
#[case("plain paragraph", true)]
#[case("2024 revenue", true)]
#[case("Text with > inside", true)]
#[case("Text with > inside | here", true)]
fn classifies_setext_text(#[case] payload: &str, #[case] expected: bool) {
    let matcher = LinkReferenceMatcher::production();
    assert_eq!(is_setext_text(payload, matcher), expected);
}

/// Asserts a digit-prefixed paragraph still converts, as before.
#[rstest]
#[case(vec!["2024 revenue".into(), "===".into()], vec!["# 2024 revenue".into()])]
fn converts_digit_prefixed_paragraphs(#[case] input: Vec<String>, #[case] expected: Vec<String>) {
    assert_eq!(convert_setext_headings(&input), expected);
}
