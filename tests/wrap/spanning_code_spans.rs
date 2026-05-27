//! Regression tests for inline code spans soft-wrapped across source lines.

use mdtablefix::wrap::wrap_text;
use rstest::rstest;

use super::*;

#[test]
fn test_wrap_spanning_code_span_bullet_fixture() {
    let input: Vec<String> = include_lines!("../data/spanning_code_span_bullet_input.txt");
    let expected: Vec<String> = include_lines!("../data/spanning_code_span_bullet_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
    assert_wrapped_list_item(&output, "- ", expected.len());
}

#[test]
fn test_wrap_spanning_code_span_ordered_fixture() {
    let input: Vec<String> = include_lines!("../data/spanning_code_span_ordered_input.txt");
    let expected: Vec<String> = include_lines!("../data/spanning_code_span_ordered_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
    assert_wrapped_list_item(&output, "1. ", expected.len());
}

#[test]
fn test_wrap_spanning_code_span_three_line_blockquote() {
    let input = lines_vec![
        "> Theme selection uses layered configuration (`CLI >",
        ">  environment > config file >",
        ">  defaults`) with OrthoConfig-backed parsing.",
    ];
    let output = process_stream(&input);
    assert!(output.len() >= 2);
    assert!(output.iter().all(|line| line.starts_with('>')));
    let rendered = output.join("\n");
    assert!(rendered.contains("(`CLI > environment > config file > defaults`)"));
}

#[test]
fn test_wrap_spanning_code_span_footnote_definition() {
    let input = lines_vec![
        "[^note]: The default interpreter is `powershell -Command` on Windows and",
        "  `bash` elsewhere unless overridden by the manifest `interpreter` field.",
    ];
    let output = process_stream(&input);
    assert!(output.len() >= 2);
    assert!(output[0].starts_with("[^note]: "));
    let rendered = output.join("\n");
    assert!(rendered.contains("`powershell -Command`"));
    assert!(rendered.contains("`bash`"));
    assert!(rendered.contains("`interpreter`"));
}

#[rstest]
#[case("- `open", " span` closes.", 1_usize)]
fn test_wrap_joins_unclosed_span_continuation(
    #[case] first: &str,
    #[case] second: &str,
    #[case] expected_lines: usize,
) {
    let input = lines_vec![first, second];
    let output = wrap_text(&input, 80);
    assert_eq!(output.len(), expected_lines);
    assert!(output[0].contains("`open span`"));
}

#[rstest]
#[case("- `done` and `open", " span` continues.")]
fn test_wrap_defers_while_any_span_stays_open(#[case] first: &str, #[case] second: &str) {
    let input = lines_vec![first, second];
    let output = wrap_text(&input, 80);
    let rendered = output.join("\n");
    assert!(rendered.contains("`done`"));
    assert!(rendered.contains("`open span`"));
}
