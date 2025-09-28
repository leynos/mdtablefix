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

#[test]
fn wrap_respects_fences_with_info_strings_and_whitespace() {
    let intro = concat!(
        "The introductory paragraph needs enough length to force wrapping so that we can confirm ",
        "behaviour when the subsequent fence appears with indentation."
    );
    let outro = concat!(
        "The final paragraph is equally verbose to verify that wrapping resumes immediately after ",
        "the closing fence with trailing spaces."
    );
    let rust_line = concat!(
        "    println!(\"This line deliberately exceeds eighty characters to prove that wrapping ",
        "remains disabled inside the fenced block.\");"
    );
    let json_line = concat!(
        "{ \"message\": \"This JSON object should stay on one line even though it is wordy\" }"
    );
    let input = lines_vec![
        intro,
        "    ```rust   lineno=1",
        rust_line,
        "    ```   ",
        "```json   ",
        json_line,
        "```   ",
        outro,
    ];
    let output = process_stream(&input);

    let fence_lines: Vec<_> = output
        .iter()
        .filter(|line| line.trim_start().starts_with("```"))
        .collect();
    assert_eq!(
        fence_lines.len(),
        4,
        "expected both opening and closing fences to be retained"
    );

    assert!(
        output.contains(&rust_line.to_string()),
        "indented code lines should remain unchanged"
    );
    assert!(
        output.contains(&json_line.to_string()),
        "info string fences should keep their payload intact"
    );

    let before_fence: Vec<_> = output
        .iter()
        .take_while(|line| !line.trim_start().starts_with("```"))
        .collect();
    assert!(
        before_fence.len() > 1,
        "introductory paragraph should wrap before the fence"
    );

    let after_fence: Vec<_> = output
        .iter()
        .rev()
        .take_while(|line| !line.trim_start().starts_with("```"))
        .collect();
    assert!(
        after_fence.len() > 1,
        "closing paragraph should wrap after the fence"
    );
}

#[test]
fn wrap_does_not_close_on_shorter_closing_marker() {
    let intro = concat!(
        "This paragraph intentionally spans more than eighty characters so that wrapping occurs ",
        "before the fenced block."
    );
    let code_line = concat!(
        "print(\"short marker test that remains inside the code fence even when the closing marker ",
        "is too short\")"
    );
    let long_code_after_short = concat!(
        "print(\"this line should stay intact because the shorter closing fence should not end the ",
        "block prematurely even though the content is wide\")"
    );
    let outro = concat!(
        "After the fence we expect wrapping to resume, demonstrating that the tracker only closes ",
        "when a marker of adequate length appears."
    );
    let input = lines_vec![
        intro,
        "````python",
        code_line,
        "```",
        long_code_after_short,
        "````",
        outro,
    ];
    let output = process_stream(&input);

    let long_line_count = output
        .iter()
        .filter(|line| line.contains("should stay intact"))
        .count();
    assert_eq!(
        long_line_count, 1,
        "long code lines after the shorter closing marker must remain unwrapped inside the fence"
    );

    let fence_lines: Vec<_> = output
        .iter()
        .filter(|line| line.trim_start().starts_with("```"))
        .collect();
    assert_eq!(
        fence_lines.len(),
        3,
        "all fence markers, including the ignored shorter one, should be retained"
    );

    let closing_idx = output
        .iter()
        .rposition(|line| line.trim_start().starts_with("````"))
        .expect("closing fence should exist");
    let post_fence = &output[closing_idx + 1..];
    assert!(
        post_fence.len() > 1,
        "paragraph after the fence should resume wrapping"
    );
    assert!(
        post_fence
            .iter()
            .all(|line| !line.trim_start().starts_with("```")),
        "trailing paragraph must not be treated as fenced content"
    );
}


