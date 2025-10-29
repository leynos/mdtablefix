//! End-to-end tests exercising footnote conversion.
//!
//! Each test processes a complete Markdown document using
//! `convert_footnotes`. Inputs are loaded from fixture files through the
//! `include_lines!` and `lines_vec!` macros re-exported by `tests::prelude`.
//! The cases mix headings, code blocks and ordinary text to confirm that
//! inline references become footnote links; eligible trailing numeric lists are
//! rewritten as definition-style footnotes when at least one footnote reference exists;
//! footnotes are renumbered sequentially with definitions reordered to match.
//!
//! A simple check ensures these macros are available so the prelude exports
//! are correctly wired for all integration tests.

use mdtablefix::{convert_footnotes, process_stream};
use rstest::rstest;

#[macro_use]
mod prelude;

#[test]
fn macros_available() {
    let _: Vec<String> = lines_vec!("a", "b");
    let _: Vec<String> = include_lines!("data/footnotes_input.txt");
}

#[test]
fn test_convert_bare_footnotes() {
    let input: Vec<String> = include_lines!("data/footnotes_input.txt");
    let expected: Vec<String> = include_lines!("data/footnotes_expected.txt");
    let output = convert_footnotes(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_idempotent_on_converted() {
    let expected: Vec<String> = include_lines!("data/footnotes_expected.txt");
    let output = convert_footnotes(&expected);
    assert_eq!(output, expected);
}

#[test]
fn test_avoids_false_positives() {
    let input = lines_vec!("Plan9 is interesting.", "Call 1-800-555-1234 for help.",);
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_ignores_numbers_in_inline_code() {
    let input = lines_vec!("Look at `code 1` for details.");
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_ignores_numbers_in_parentheses() {
    let input = lines_vec!("Refer to equation (1) for context.");
    assert_eq!(convert_footnotes(&input), input);
}

#[rstest]
#[case("### A.2 A Note on This List")]
#[case("### Heading with footnote[1]")]
#[case("> ### A.2 A Note on This List")]
#[case("- ### A.2 A Note on This List")]
#[case("* ### A.2 A Note on This List")]
#[case("+ ### A.2 A Note on This List")]
#[case("1. ### A.2 A Note on This List")]
#[case("1) ### A.2 A Note on This List")]
#[case("- 1. ### A.2 A Note on This List")]
#[case(">> ### A.2 A Note on This List")]
#[case(">>> ### A.2 A Note on This List")]
fn heading_lines_are_left_verbatim(#[case] line: &str) {
    let input = lines_vec!(line);
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_ignores_numbers_in_fenced_code_block() {
    let input = lines_vec!(
        "Here is a code block:",
        "```",
        "let x = 42; // note 1",
        "```",
        "Done."
    );
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_ignores_numbers_in_indented_code_block() {
    let input = lines_vec!(
        "    let a = 1;",
        "    let b = 2; // number 2",
        "",
        "Outside."
    );
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_handles_punctuation_inside_bold() {
    let input = lines_vec!("It was **scary.**7");
    let expected = lines_vec!("It was **scary.**[^1]");
    assert_eq!(convert_footnotes(&input), expected);
}

#[rstest]
#[case(
    lines_vec!(
        "While a full library tutorial is beyond this guide's scope, a brief look at the",
        "core API concepts reveals its ergonomic design. The official `docs.rs` page",
        "provides several end-to-end examples that revolve around a few key types 7:",
    ),
    lines_vec!(
        "While a full library tutorial is beyond this guide's scope, a brief look at the",
        "core API concepts reveals its ergonomic design. The official `docs.rs` page",
        "provides several end-to-end examples that revolve around a few key types[^1]:",
    )
)]
#[case(
    lines_vec!(
        "This is a reference 12:: to something important.",
        "Another example 3::: with more colons.",
    ),
    lines_vec!(
        "This is a reference[^1]:: to something important.",
        "Another example[^2]::: with more colons.",
    )
)]
#[case(
    lines_vec!(
        "See the details in section 5:, which are crucial.",
        "Another case is 8:; for completeness.",
    ),
    lines_vec!(
        "See the details in section[^1]:, which are crucial.",
        "Another case is[^2]:; for completeness.",
    )
)]
#[case(
    lines_vec!(
        "This is a tricky one  9 : and should be handled.",
        "Extra spaces  10  : are also possible.",
    ),
    lines_vec!(
        "This is a tricky one[^1]: and should be handled.",
        "Extra spaces[^2]: are also possible.",
    )
)]
fn test_converts_number_followed_by_colon(
    #[case] input: Vec<String>,
    #[case] expected: Vec<String>,
) {
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn test_converts_colon_footnote_definition() {
    let input = lines_vec!("## Footnotes", "7: Footnote text");
    let expected = lines_vec!("## Footnotes", "[^1]: Footnote text");
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn test_converts_colon_definition_with_leading_spaces() {
    let input = lines_vec!("## Footnotes", "  7: Footnote text");
    let expected = lines_vec!("## Footnotes", "  [^1]: Footnote text");
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn test_converts_colon_definition_with_trailing_spaces() {
    let input = lines_vec!("## Footnotes", "7:  Footnote text");
    let expected = lines_vec!("## Footnotes", "[^1]:  Footnote text");
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn test_convert_preserves_headers_with_blank_separators() {
    let input: Vec<String> = include_lines!("data/footnotes_regression_input.txt");
    let expected: Vec<String> = include_lines!("data/footnotes_regression_expected.txt");

    let reflowed = process_stream(&input);
    assert_eq!(
        reflowed, expected,
        "reflowed fixture must match expectation"
    );

    let output = convert_footnotes(&reflowed);
    assert_eq!(output, expected);
}

#[test]
fn test_converts_list_with_blank_lines() {
    let input = lines_vec!(
        "Text.",
        "",
        "## Footnotes",
        "",
        " 1. First",
        "  ",
        " 2. Second",
        "",
        "10. Tenth",
        "   ",
        "",
    );
    let expected = lines_vec!(
        "Text.",
        "",
        "## Footnotes",
        "",
        " [^1]: First",
        "  ",
        " [^2]: Second",
        "",
        "[^3]: Tenth",
        "   ",
        "",
    );
    let output = convert_footnotes(&input);
    assert_eq!(output, expected);
}

#[test]
fn test_empty_input() {
    let input: Vec<String> = Vec::new();
    let output = convert_footnotes(&input);
    assert!(output.is_empty());
}

#[test]
fn test_whitespace_input() {
    let input = lines_vec!("   ", "\t");
    let output = convert_footnotes(&input);
    assert_eq!(output, input);
}

#[test]
fn test_requires_h2_heading() {
    let input = lines_vec!("Text.", " 1. First footnote",);
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_skips_when_existing_block_present() {
    let input = lines_vec!("[^1]: Old", "## Footnotes", " 2. New",);
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_skips_when_list_not_last() {
    let input = lines_vec!("## Footnotes", " 1. Note", "", "Tail.",);
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_skips_with_h3_heading() {
    let input = lines_vec!("Text.", "### Notes", " 1. First");
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_converts_with_non_footnotes_h2() {
    let input = lines_vec!("## Notes", " 1. First");
    let expected = lines_vec!("## Notes", " [^1]: First");
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn test_skips_when_existing_block_is_indented_or_quoted() {
    let input1 = lines_vec!("  [^1]: Old", "## Footnotes", " 2. New");
    let input2 = lines_vec!("> [^1]: Old", "## Footnotes", " 2. New");
    let input3 = lines_vec!(">> [^1]: Old", "## Footnotes", " 2. New");
    assert_eq!(convert_footnotes(&input1), input1);
    assert_eq!(convert_footnotes(&input2), input2);
    assert_eq!(convert_footnotes(&input3), input3);
}

#[test]
fn test_converts_after_inline_reference_at_bol() {
    let input = lines_vec!("[^1] see note", "## Footnotes", " 1. First");
    let expected = lines_vec!("[^1] see note", "## Footnotes", " [^1]: First");
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn test_ignores_definition_inside_fence() {
    let input = lines_vec!("```", "[^1]: Old", "```", "## Footnotes", " 1. First",);
    let expected = lines_vec!("```", "[^1]: Old", "```", "## Footnotes", " [^1]: First",);
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn test_renumbers_numeric_list_without_heading() {
    let input = lines_vec!(
        "First reference.[^7]",
        "Second reference.[^3]",
        "",
        "1. Legacy footnote",
        "3. Third footnote",
        "7. Seventh footnote",
    );
    let expected = lines_vec!(
        "First reference.[^1]",
        "Second reference.[^2]",
        "",
        "[^1]: Seventh footnote",
        "[^2]: Third footnote",
        "[^3]: Legacy footnote",
    );
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn test_renumbers_numeric_list_with_wrapped_items_and_duplicates_without_heading() {
    let input = lines_vec!(
        "First ref.[^7] and again [^7]",
        "",
        "1. Legacy footnote",
        "3. Third footnote wraps",
        "   over two lines.",
        "7. Seventh footnote",
    );
    let expected = lines_vec!(
        "First ref.[^1] and again [^1]",
        "",
        "[^1]: Seventh footnote",
        "[^2]: Third footnote wraps",
        "   over two lines.",
        "[^3]: Legacy footnote",
    );
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn test_skips_numeric_list_not_last_without_heading() {
    let input = lines_vec!(
        "Reference.[^2]",
        "1. First footnote",
        "2. Second footnote",
        "",
        "Tail.",
    );
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn test_renumbers_reference_followed_by_colons() {
    let input = lines_vec!(
        "Usage.[^7]:: extra context",
        "",
        "## Footnotes",
        "7. Footnote text",
    );
    let expected = lines_vec!(
        "Usage.[^1]:: extra context",
        "",
        "## Footnotes",
        "[^1]: Footnote text",
    );
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn test_preserves_numeric_list_without_references() {
    let input = lines_vec!("Ordinary list:", "1. Apples", "2. Bananas",);
    assert_eq!(convert_footnotes(&input), input);
}
