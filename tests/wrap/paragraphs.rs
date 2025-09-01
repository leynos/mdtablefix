//! Paragraph wrapping tests.
//!
//! Validates text wrapping behaviour for paragraph content, including handling
//! of long words that exceed the 80-column limit and cannot be broken.

use rstest::rstest;
use super::*;

#[test]
fn test_wrap_paragraph() {
    let input = lines_vec![
        "This is a very long paragraph that should be wrapped at eighty columns so it needs to \
         contain enough words to exceed that limit.",
    ];
    let output = process_stream(&input);
    assert!(output.len() > 1);
    assert!(output.iter().all(|l| l.len() <= 80));
}

#[test]
#[rstest]
#[case(100)]
#[case(150)]
#[case(200)]
fn test_wrap_paragraph_with_long_word_parameterised(#[case] word_length: usize) {
    let long_word = "a".repeat(word_length);
    let input = lines_vec![&long_word];
    let output = process_stream(&input);
    assert_eq!(output.len(), 1);
    assert_eq!(output[0], long_word);
}

#[test]
fn test_wrap_preserves_inline_code_with_trailing_punctuation() {
    let input: Vec<String> = include_lines!("data/fsm_paragraph_input.txt");
    let expected: Vec<String> = include_lines!("data/fsm_paragraph_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
}

#[rstest]
#[case("`useState`.")]
#[case("`useState`,")]
#[case("`useState`!")]
#[case("`useState`?")]
#[case("`useState`”")]
#[case("`useState`’")]
#[case("`useState`）")]
#[case("`useState`。")]
#[case("`useState`…")]
#[case("`useState`?!")]
#[case("`isError?`.")]
fn test_wrap_inline_code_trailing_punct_cases(#[case] snippet: &str) {
    let prefix =
        "This line is long enough that wrapping will occur near the end, ensuring ";
    let input = lines_vec![&format!("{prefix}{snippet}")];
    let output = process_stream(&input);
    // Ensure the snippet remains intact and not split between lines.
    assert!(output.iter().any(|l| l.contains(snippet)));
}
