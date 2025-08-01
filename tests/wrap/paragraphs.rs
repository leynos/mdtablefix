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
