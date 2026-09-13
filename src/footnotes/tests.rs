//! Unit tests for footnote conversion.

use rstest::rstest;

use super::{convert_footnote_definitions, convert_footnotes, convert_inline_footnotes};

#[test]
fn converts_inline_numbers() {
    let input = vec!["See the docs.2".to_string()];
    let expected = vec!["See the docs.[^1]".to_string()];
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn converts_final_list() {
    let input = vec![
        "Text.".to_string(),
        String::new(),
        "## Footnotes".to_string(),
        String::new(),
        " 1. First".to_string(),
        " 2. Second".to_string(),
    ];
    let expected = vec![
        "Text.".to_string(),
        String::new(),
        "## Footnotes".to_string(),
        String::new(),
        " [^1]: First".to_string(),
        " [^2]: Second".to_string(),
    ];
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn converts_list_with_blank_lines() {
    let input = vec![
        "Text.".to_string(),
        String::new(),
        "## Footnotes".to_string(),
        String::new(),
        " 1. First".to_string(),
        String::new(),
        " 2. Second".to_string(),
        String::new(),
        "10. Tenth".to_string(),
    ];
    let expected = vec![
        "Text.".to_string(),
        String::new(),
        "## Footnotes".to_string(),
        String::new(),
        " [^1]: First".to_string(),
        String::new(),
        " [^2]: Second".to_string(),
        String::new(),
        "[^3]: Tenth".to_string(),
    ];
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn idempotent_on_existing_block() {
    let input = vec![" [^1]: First".to_string()];
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn skips_with_existing_block() {
    let input = vec![
        "[^1]: Old".to_string(),
        "## Footnotes".to_string(),
        " 2. New".to_string(),
    ];
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn skips_without_h2() {
    let input = vec!["Text.".to_string(), " 1. First".to_string()];
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn skips_when_list_not_last() {
    let input = vec![
        "## Footnotes".to_string(),
        " 1. First".to_string(),
        String::new(),
        "Tail.".to_string(),
    ];
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn skips_when_block_has_only_blanks() {
    let input = vec!["## Footnotes".to_string(), String::new()];
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn multiple_inline_notes_in_one_line() {
    let input = vec!["First.1 Then?2".to_string()];
    let expected = vec!["First.[^1] Then?[^2]".to_string()];
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn ignores_non_numeric_footnote_block() {
    let input = vec!["Text.".to_string(), " a. note".to_string()];
    assert_eq!(convert_footnotes(&input), input);
}

#[test]
fn empty_input_returns_empty_vec() {
    let input: Vec<String> = Vec::new();
    assert!(convert_footnotes(&input).is_empty());
}

#[test]
fn converts_only_final_contiguous_block() {
    let input = vec![
        "Intro.".to_string(),
        "1. not a footnote".to_string(),
        "More text.".to_string(),
        "## Footnotes".to_string(),
        "2. final".to_string(),
    ];
    let expected = vec![
        "Intro.".to_string(),
        "1. not a footnote".to_string(),
        "More text.".to_string(),
        "## Footnotes".to_string(),
        "[^1]: final".to_string(),
    ];
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn renumbers_references_and_definitions() {
    let input = vec![
        "First reference.[^7]".to_string(),
        "Second reference.[^3]".to_string(),
        String::new(),
        "  [^3]: Third footnote".to_string(),
        "  [^7]: Seventh footnote".to_string(),
    ];
    let expected = vec![
        "First reference.[^1]".to_string(),
        "Second reference.[^2]".to_string(),
        String::new(),
        "  [^1]: Seventh footnote".to_string(),
        "  [^2]: Third footnote".to_string(),
    ];
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn preserves_multiline_definition_blocks() {
    let input = vec![
        "Intro.[^2]".to_string(),
        String::new(),
        "[^1]: Legacy footnote".to_string(),
        "    More legacy context.".to_string(),
        String::new(),
        "[^2]: Current footnote".to_string(),
        "    Additional context.".to_string(),
    ];
    let expected = vec![
        "Intro.[^1]".to_string(),
        String::new(),
        "[^1]: Current footnote".to_string(),
        "    Additional context.".to_string(),
        String::new(),
        "[^2]: Legacy footnote".to_string(),
        "    More legacy context.".to_string(),
    ];
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn assigns_new_numbers_to_unreferenced_definitions() {
    let input = vec![
        "Alpha.[^5]".to_string(),
        "Beta.[^2]".to_string(),
        String::new(),
        "[^1]: Legacy footnote".to_string(),
        "[^2]: Beta footnote".to_string(),
        "[^5]: Alpha footnote".to_string(),
    ];
    let expected = vec![
        "Alpha.[^1]".to_string(),
        "Beta.[^2]".to_string(),
        String::new(),
        "[^1]: Alpha footnote".to_string(),
        "[^2]: Beta footnote".to_string(),
        "[^3]: Legacy footnote".to_string(),
    ];
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn updates_references_inside_definitions() {
    let input = vec![
        "Intro.[^4]".to_string(),
        String::new(),
        "[^4]: See [^2] for context".to_string(),
        "[^2]: Base note".to_string(),
    ];
    let expected = vec![
        "Intro.[^1]".to_string(),
        String::new(),
        "[^1]: See [^2] for context".to_string(),
        "[^2]: Base note".to_string(),
    ];
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn renumbers_numeric_list_without_heading() {
    let input = vec![
        "First reference.[^7]".to_string(),
        "Second reference.[^3]".to_string(),
        String::new(),
        "1. Legacy footnote".to_string(),
        "3. Third footnote".to_string(),
        "7. Seventh footnote".to_string(),
    ];
    let expected = vec![
        "First reference.[^1]".to_string(),
        "Second reference.[^2]".to_string(),
        String::new(),
        "[^1]: Seventh footnote".to_string(),
        "[^2]: Third footnote".to_string(),
        "[^3]: Legacy footnote".to_string(),
    ];
    assert_eq!(convert_footnotes(&input), expected);
}

#[test]
fn leaves_numeric_list_without_references_unchanged() {
    let input = vec![
        "Ordinary list:".to_string(),
        "1. Apples".to_string(),
        "2. Bananas".to_string(),
    ];
    assert_eq!(convert_footnotes(&input), input);
}

/// Asserts the inline stage rewrites bare numeric references and leaves the
/// label stage's work alone.
///
/// `process_stream_inner` calls the stages separately, each side of the passes
/// that measure text, so each has to stop at its own half; a whole-document
/// test cannot see one stage taking on the other's work. The inline stage
/// writes the number the reference was written with — `docs.2` becomes
/// `[^2]` — and leaves the sequencing to `renumber_footnote_labels`, which
/// numbers the distinct references by first encounter.
#[rstest]
#[case::bare_reference_keeps_its_written_number(vec!["See docs.2"], vec!["See docs.[^2]"])]
#[case::label_untouched(vec!["Intro.[^7]"], vec!["Intro.[^7]"])]
#[case::definition_untouched(vec![" [^1]: First"], vec![" [^1]: First"])]
#[case::ordered_item_untouched(vec![" 1. First"], vec![" 1. First"])]
fn inline_stage_rewrites_only_bare_references(
    #[case] input: Vec<&str>,
    #[case] expected: Vec<&str>,
) {
    assert_eq!(convert_inline_footnotes(&owned(&input)), owned(&expected));
}

/// Asserts the definition stage settles the definition block and leaves a
/// bare numeric reference to the inline stage that runs before it.
#[rstest]
#[case::bare_reference_untouched(vec!["See docs.2"], vec!["See docs.2"])]
#[case::labels_renumbered_and_block_reordered(
    vec![
        "First reference.[^7]",
        "Second reference.[^3]",
        "",
        "  [^3]: Third footnote",
        "  [^7]: Seventh footnote",
    ],
    vec![
        "First reference.[^1]",
        "Second reference.[^2]",
        "",
        "  [^1]: Seventh footnote",
        "  [^2]: Third footnote",
    ],
)]
fn definition_stage_leaves_bare_references_to_the_inline_stage(
    #[case] input: Vec<&str>,
    #[case] expected: Vec<&str>,
) {
    assert_eq!(
        convert_footnote_definitions(&owned(&input)),
        owned(&expected),
    );
}

/// Owns a case's lines for the stages, which take `&[String]`.
fn owned(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_string()).collect()
}
