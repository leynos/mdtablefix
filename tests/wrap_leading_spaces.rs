//! Regression and property tests for issue `#291` leading-space reflow bugs.
//!
//! Guards against spurious carry whitespace at the start of wrapped continuation
//! lines while preserving legitimate list-item continuation indents.

#[macro_use]
#[path = "common/mod.rs"]
mod common;

use std::sync::LazyLock;

use mdtablefix::{lazy_regex, process_stream};
use proptest::prelude::*;
use regex::Regex;
use unicode_width::UnicodeWidthStr;

static BULLET_RE: LazyLock<Regex> = lazy_regex!(
    r"^(\s*(?:[-*+]|\d+[.)])\s+(?:\[\s*(?:[xX]|\s)\s*\]\s*)?)(.*)",
    "bullet pattern regex should compile",
);

/// Returns the expected continuation indent for a wrapped list item output.
fn list_continuation_indent(first_line: &str) -> Option<String> {
    let prefix = BULLET_RE.captures(first_line)?.get(1)?.as_str();
    let indent_str: String = prefix.chars().take_while(|c| c.is_whitespace()).collect();
    let prefix_width = UnicodeWidthStr::width(prefix);
    let indent_width = UnicodeWidthStr::width(indent_str.as_str());
    Some(format!(
        "{}{}",
        indent_str,
        " ".repeat(prefix_width.saturating_sub(indent_width))
    ))
}

/// Asserts wrapped output does not introduce spurious leading spaces.
fn assert_no_spurious_leading_spaces(output: &[String]) {
    assert!(!output.is_empty(), "output must not be empty");

    let list_indent = output
        .first()
        .map(String::as_str)
        .and_then(list_continuation_indent)
        .unwrap_or_default();

    for (index, line) in output.iter().enumerate() {
        if line.is_empty() {
            continue;
        }

        if index == 0 || list_indent.is_empty() {
            assert!(
                !(line.starts_with(' ') && list_indent.is_empty()),
                "plain paragraph line must not begin with a space: {line:?}"
            );
            continue;
        }

        assert!(
            line.starts_with(&list_indent),
            "list continuation line must preserve indent {list_indent:?}: {line:?}"
        );
        let content = &line[list_indent.len()..];
        assert!(
            !content.starts_with(' '),
            "spurious carry whitespace after list indent on line {index}: {line:?}"
        );
    }
}

#[test]
fn wrap_issue_291_execplan_fixture() {
    let input: Vec<String> = include_lines!("data/issue_291_execplan_input.txt");
    let expected: Vec<String> = include_lines!("data/issue_291_execplan_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
    assert_no_spurious_leading_spaces(&output);
}

#[test]
fn wrap_issue_291_errors_fixture() {
    let input: Vec<String> = include_lines!("data/issue_291_errors_input.txt");
    let expected: Vec<String> = include_lines!("data/issue_291_errors_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
    assert_no_spurious_leading_spaces(&output);
}

#[test]
fn wrap_issue_291_netsuke_list_fixture() {
    let input: Vec<String> = include_lines!("data/issue_291_netsuke_list_input.txt");
    let expected: Vec<String> = include_lines!("data/issue_291_netsuke_list_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
    assert_no_spurious_leading_spaces(&output);
}

#[test]
fn wrap_issue_291_kani_list_fixture() {
    let input: Vec<String> = include_lines!("data/issue_291_kani_list_input.txt");
    let expected: Vec<String> = include_lines!("data/issue_291_kani_list_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
    assert_no_spurious_leading_spaces(&output);
}

#[test]
fn wrap_issue_291_ordered_list_fixture() {
    let input: Vec<String> = include_lines!("data/issue_291_ordered_list_input.txt");
    let expected: Vec<String> = include_lines!("data/issue_291_ordered_list_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
    assert_no_spurious_leading_spaces(&output);
}

#[test]
fn wrap_issue_291_nested_ordered_list_fixture() {
    let input: Vec<String> = include_lines!("data/issue_291_nested_ordered_list_input.txt");
    let expected: Vec<String> = include_lines!("data/issue_291_nested_ordered_list_expected.txt");
    let output = process_stream(&input);
    assert_eq!(output, expected);
    assert_no_spurious_leading_spaces(&output);
}

fn prose_word_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            Just("alpha".to_string()),
            Just("beta".to_string()),
            Just("gamma".to_string()),
            Just("delta".to_string()),
            Just("epsilon".to_string()),
            Just("zeta".to_string()),
            Just("observation".to_string()),
            Just("evidence".to_string()),
            Just("discovery".to_string()),
        ],
        8..20,
    )
    .prop_map(|words| words.join(" "))
}

fn inline_code_span_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            Just("Constraints".to_string()),
            Just("Tolerances".to_string()),
            Just("errors".to_string()),
            Just("kani-smoke".to_string()),
            Just("4.1.2".to_string()),
            Just("4.2.*".to_string()),
            Just("--file".to_string()),
        ],
        1..4,
    )
    .prop_map(|labels| {
        labels
            .into_iter()
            .map(|label| format!("`{label}`"))
            .collect::<Vec<_>>()
            .join(", ")
    })
}

fn list_prefix_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("- ".to_string()),
        Just("* ".to_string()),
        Just("+ ".to_string()),
        Just("- [ ] ".to_string()),
        Just("- [x] ".to_string()),
        (1usize..3).prop_map(|n| format!("{n}. ")),
        (10usize..20).prop_map(|n| format!("{n}. ")),
        (1usize..3).prop_map(|n| format!("{}  - ", "  ".repeat(n))),
        (1usize..3).prop_map(|n| format!("{}  {}. ", "  ".repeat(n), n + 1)),
    ]
}

proptest! {
    #[test]
    fn wrap_paragraphs_do_not_introduce_leading_spaces(
        prefix in prose_word_strategy(),
        code in inline_code_span_strategy(),
        suffix in prose_word_strategy(),
    ) {
        let body = format!("{prefix} {code} {suffix}");
        let input = vec![body];
        let output = process_stream(&input);
        assert_no_spurious_leading_spaces(&output);
        prop_assert!(output.iter().all(|line| line.len() <= 80));
    }

    #[test]
    fn wrap_list_items_do_not_introduce_spurious_leading_spaces(
        prefix in list_prefix_strategy(),
        prose in prose_word_strategy(),
        code in inline_code_span_strategy(),
        tail in prose_word_strategy(),
    ) {
        let body = format!("{prose} {code} {tail}");
        let line = format!("{prefix}{body}");
        let leading_spaces = line.chars().take_while(|&c| c == ' ').count();
        prop_assume!(leading_spaces < 4);
        prop_assume!(line.len() > 80);
        let input = vec![line];
        let output = process_stream(&input);
        prop_assert!(output.len() > 1, "expected wrapping to occur");
        prop_assert!(
            output[0].starts_with(&prefix),
            "list prefix must be preserved on the first line"
        );
        prop_assert!(
            output.iter().all(|line| line.len() <= 80),
            "wrapped lines must not exceed 80 columns"
        );
        assert_no_spurious_leading_spaces(&output);
    }
}
