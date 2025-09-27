//! Behavioural tests for fence-aware wrapping.

use super::*;

#[test]
fn wrap_respects_fence_boundaries_in_paragraphs() {
    let first_paragraph = concat!(
        "This introductory paragraph is intentionally verbose to ensure that wrapping ",
        "is required before reaching the fenced code block, demonstrating how the ",
        "tracker suspends prose formatting once a fence begins.",
    );
    let closing_paragraph = concat!(
        "This closing paragraph is equally loquacious so that we can prove wrapping ",
        "resumes immediately after the fenced block without altering the code content.",
    );
    let code_line = concat!(
        "fn demonstrate() { println!(\"This code line intentionally exceeds eighty characters ",
        "to ensure the wrapping logic would normally split it if fences were not honoured.\"); }",
    );
    let input = lines_vec![first_paragraph, "```", code_line, "```", closing_paragraph];
    let output = process_stream(&input);
    assert!(!output.iter().any(|l| l.starts_with("``") && l.len() == 2), "no false 2-tick fences");

    let fence_positions: Vec<usize> = output
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| (line == "```").then_some(idx))
        .collect();
    assert_eq!(fence_positions.len(), 2, "expected exactly two fence markers");
    assert!(output.contains(&code_line.to_string()), "expected code line to remain intact");

    let before_fence = &output[..fence_positions[0]];
    assert!(before_fence.len() > 1, "prose before the fence should wrap");

    let after_fence = &output[fence_positions[1] + 1..];
    assert!(after_fence.len() > 1, "prose after the fence should resume wrapping");
    assert!(
        after_fence
            .iter()
            .any(|line| line.contains("closing paragraph is equally loquacious")),
        "expected trailing paragraph content after fence",
    );
}

