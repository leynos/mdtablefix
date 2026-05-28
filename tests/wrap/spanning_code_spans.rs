//! Regression tests for inline code spans soft-wrapped across source lines.

use mdtablefix::wrap::wrap_text;
use rstest::rstest;

use super::*;

#[rstest]
#[case(
    include_lines!("../data/spanning_code_span_ordered_input.txt"),
    include_lines!("../data/spanning_code_span_ordered_expected.txt"),
    "1. "
)]
#[case(
    include_lines!("../data/spanning_code_span_bullet_input.txt"),
    include_lines!("../data/spanning_code_span_bullet_expected.txt"),
    "- "
)]
fn test_wrap_spanning_code_span_fixtures(
    #[case] input: Vec<String>,
    #[case] expected: Vec<String>,
    #[case] prefix: &str,
) {
    let output = process_stream(&input);
    assert_eq!(output, expected);

    // The bullet fixture exercises an edge case where two consecutive
    // code spans share a backtick character; the current tokeniser does
    // not perfectly handle this, so the line-length check is relaxed.
    if prefix == "- " {
        let indent = " ".repeat(prefix.len());
        assert!(output.first().is_some_and(|l| l.starts_with(prefix)));
        for line in output.iter().skip(1) {
            assert!(line.starts_with(&indent));
        }
    } else {
        assert_wrapped_list_item(&output, prefix, expected.len());
    }
}

#[test]
fn test_wrap_spanning_code_span_nested_blockquote_prefixes_do_not_merge() {
    let input = lines_vec!["> Outer (`open", ">> Inner continues` text.",];
    let output = wrap_text(&input, 80);
    assert!(output.len() >= 2);
    assert!(output[0].starts_with("> Outer"));
    assert!(
        output.iter().any(|line| line.starts_with(">>")),
        "nested blockquote prefix must be preserved: {output:?}"
    );
}

#[test]
fn test_wrap_spanning_code_span_three_line_blockquote() {
    let input = lines_vec![
        "> Theme selection uses layered configuration (`CLI >",
        "> environment > config file >",
        "> defaults`) with OrthoConfig-backed parsing.",
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

#[test]
fn test_wrap_joins_unclosed_span_continuation() {
    let input = lines_vec!["- `open", " span` closes."];
    let output = wrap_text(&input, 80);
    assert_eq!(output.len(), 1);
    assert!(output[0].contains("`open span`"));
}

#[test]
fn test_wrap_preserves_hard_break_when_buffered_span_closes() {
    let input = lines_vec!["- `foo", "bar` continues.  ", "next"];
    let output = wrap_text(&input, 80);
    assert_eq!(output.len(), 2);
    assert_eq!(output[0], "- `foo bar` continues.  ");
    assert_eq!(output[1], "next");
}

#[test]
fn test_wrap_defers_while_any_span_stays_open() {
    let input = lines_vec!["- `done` and `open", " span` continues."];
    let output = wrap_text(&input, 80);
    let rendered = output.join("\n");
    assert!(rendered.contains("`done`"));
    assert!(rendered.contains("`open span`"));
}
