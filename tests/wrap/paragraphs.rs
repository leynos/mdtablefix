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
    let input: Vec<String> = include_lines!("../data/fsm_paragraph_input.txt");
    let expected: Vec<String> = include_lines!("../data/fsm_paragraph_expected.txt");
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
    let prefix = "This line is long enough that wrapping will occur near the end, ensuring ";
    let input = lines_vec![&format!("{prefix}{snippet}")];
    let output = process_stream(&input);
    // Ensure the snippet remains intact and not split between lines.
    assert!(output.iter().any(|l| l.contains(snippet)));
}

#[test]
fn test_wrap_inline_code_at_line_start() {
    let snippet = "`useState`.";
    let suffix = concat!(
        "This line is long enough that wrapping will occur after the trailing ",
        "punctuation, verifying the start boundary."
    );
    let input = lines_vec![format!("{snippet} {suffix}")];
    let output = process_stream(&input);
    assert!(output[0].starts_with(snippet));
}

#[test]
fn test_wrap_inline_code_surrounded_by_spaces() {
    let snippet = "`useState`.";
    let prefix = concat!(
        "This line is long enough that wrapping will occur before the inline ",
        "code"
    );
    let suffix = "demonstrating handling when code is surrounded by spaces.";
    let input = lines_vec![format!("{prefix} {snippet} {suffix}")];
    let output = process_stream(&input);
    assert!(output.iter().any(|l| l.contains(snippet)));
}

#[test]
fn test_wrap_preserves_escaped_triple_backticks() {
    let input = lines_vec![r"\`\`\`ignore"];
    let output = process_stream(&input);
    assert_eq!(output, input);
}

#[test]
fn test_wrap_no_leading_spaces_on_continuation_lines() {
    let input = lines_vec![concat!(
        "This ExecPlan (execution plan) is a living document. The sections ",
        "`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, ",
        "`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work ",
        "proceeds."
    ),];
    let output = process_stream(&input);
    assert!(output.len() > 1);
    for line in &output {
        assert!(
            !line.starts_with(' '),
            "reflowed line must not begin with a spurious leading space: {line:?}"
        );
    }
}

#[rstest]
#[case("`Constraints`, `Tolerances`, and `Risks`.")]
#[case("See `alpha` and `beta` for details.")]
#[case("(`code`) with opening parenthesis.")]
fn test_wrap_no_leading_spaces_with_inline_code(#[case] snippet: &str) {
    let prefix = concat!(
        "This paragraph is deliberately long enough to force wrapping across ",
        "multiple output lines while preserving inline code spans near the ",
        "wrap boundary. "
    );
    let input = lines_vec![format!("{prefix}{snippet}")];
    let output = process_stream(&input);
    assert!(output.len() > 1);
    for line in &output {
        assert!(
            !line.starts_with(' '),
            "reflowed line must not begin with a spurious leading space: {line:?}"
        );
    }
}

#[test]
fn test_wrap_preserves_escaped_backticks_in_paragraph() {
    let input = lines_vec![
        r"This deliberately verbose paragraph holds escaped ticks like \`code\` alongside [link](https://ex.com) markup and emphasis *still ok* so that wrapping must retain the literal ticks."
    ];
    let output = process_stream(&input);
    assert!(output.len() > 1);
    let flattened = output.join(" ");
    assert!(flattened.contains(r"\`code\`"));
    assert!(flattened.contains("[link](https://ex.com)"));
    assert!(flattened.contains("*still ok*"));
}

#[rstest]
#[case("`VarGuard`s")]
#[case("`class`'s")]
#[case("`fetch`ed")]
#[case("`run`ning")]
fn test_wrap_keeps_inline_code_suffix_on_same_line(#[case] snippet: &str) {
    let close_backtick = snippet
        .rfind('`')
        .expect("snippet must contain inline code");
    let orphaned_suffix = &snippet[close_backtick + 1..];

    let prefix = concat!(
        "When a scenario requires batched cleanup, collect the original values and ",
        "restore them with `restore_many()` from `TestWorld::drop`, or keep "
    );
    let suffix = " alive for the scenario lifetime. For BDD scenarios specifically,";
    let input = lines_vec![format!("{prefix}{snippet}{suffix}")];
    let output = process_stream(&input);
    assert!(output.len() > 1);
    for line in output.iter().skip(1) {
        let trimmed = line.trim_start();
        let orphaned_at_start = trimmed.starts_with(orphaned_suffix) && {
            let rest = &trimmed[orphaned_suffix.len()..];
            rest.is_empty()
                || rest.starts_with(|ch: char| ch.is_whitespace() || ch.is_ascii_punctuation())
        };
        assert!(
            !orphaned_at_start,
            "inflectional suffix must not appear alone at line start: {output:?}"
        );
    }
    let flattened = output.join("\n");
    assert!(flattened.contains(snippet));
}

#[rstest]
#[case("pre-`LLMPort`")]
#[case("LLM-`Port`")]
#[case("(API-`Foo`)")]
fn test_wrap_keeps_inline_code_leading_hyphen_on_same_line(#[case] snippet: &str) {
    let prefix = concat!(
        "When the scaffold includes an adapter-facing slice, document the seam ",
        "and route the behavioural coverage for that task through the "
    );
    let suffix = " interface so that the wrapping logic keeps the compound atomic.";
    let input = lines_vec![format!("{prefix}{snippet}{suffix}")];
    let output = process_stream(&input);
    assert!(output.len() > 1);
    assert!(
        output.iter().any(|line| line.contains(snippet)),
        "expected compound {snippet:?} preserved on a single line: {output:?}"
    );
}
