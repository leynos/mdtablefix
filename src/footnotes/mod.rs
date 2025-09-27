//! Footnote normalisation utilities.
//!
//! Converts bare numeric references in text to GitHub-flavoured Markdown
//! footnote links and normalises footnote numbering and ordering by
//! orchestrating specialised submodules.

mod inline;
mod lists;
mod parsing;
mod renumber;

use crate::textproc::{Token, push_original_token, tokenize_markdown};

use inline::{convert_inline, is_atx_heading_prefix};
use lists::convert_block;
use renumber::renumber_footnotes;

/// Convert bare numeric footnote references to Markdown footnote syntax.
#[must_use]
pub fn convert_footnotes(lines: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());

    for line in lines {
        if is_atx_heading_prefix(line) {
            out.push(line.clone());
        } else {
            let mut converted = String::with_capacity(line.len());
            for token in tokenize_markdown(line) {
                match token {
                    Token::Text(t) => converted.push_str(&convert_inline(t)),
                    other => push_original_token(&other, &mut converted),
                }
            }
            out.push(converted);
        }
    }

    convert_block(&mut out);
    renumber_footnotes(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::convert_footnotes;

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
}
