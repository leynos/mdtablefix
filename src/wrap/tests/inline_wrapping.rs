//! Tests for inline wrapping that preserves code spans and links.

use rstest::rstest;

use super::super::inline::{attach_punctuation_to_previous_line, wrap_preserving_code};

#[test]
fn attach_punctuation_appends_to_previous_code_line() {
    let mut lines = vec!["wrap `code`".to_string()];
    let current = String::new();
    assert!(attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        "!",
    ));
    assert_eq!(lines, vec!["wrap `code`!".to_string()]);
}

#[test]
fn attach_punctuation_requires_empty_current_buffer() {
    let mut lines = vec!["`code`".to_string()];
    let current = " pending".to_string();
    assert!(!attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        "!",
    ));
    assert_eq!(lines, vec!["`code`".to_string()]);
}

#[test]
fn attach_punctuation_ignores_non_code_suffix() {
    let mut lines = vec!["plain text".to_string()];
    let current = String::new();
    assert!(!attach_punctuation_to_previous_line(
        lines.as_mut_slice(),
        &current,
        ".",
    ));
    assert_eq!(lines, vec!["plain text".to_string()]);
}

#[test]
fn wrap_preserving_code_splits_after_consecutive_whitespace() {
    let lines = wrap_preserving_code("alpha  beta   gamma", 8);
    assert_eq!(
        lines,
        vec![
            "alpha  ".to_string(),
            "beta   ".to_string(),
            "gamma".to_string()
        ]
    );
}

#[test]
fn wrap_preserving_code_couples_opening_paren_before_inline_code() {
    let text = concat!(
        "- `src/cli/mod.rs` (240 lines): defines the `Cli` struct with ",
        "`#[derive(Parser, Serialize, Deserialize, OrthoConfig)]`, its subcommands ",
        "(`Commands` enum), and the `parse_with_localizer_from` function that creates ",
        "a localized clap command and parses arguments."
    );
    let lines = wrap_preserving_code(text, 80);
    for window in lines.windows(2) {
        assert!(
            !window[0].ends_with('('),
            "opening parenthesis must not be stranded at line end: {lines:?}"
        );
    }
}

#[rstest]
#[case("(`code`)", 10)]
#[case("[`code`]", 10)]
#[case("（`code`）", 10)]
#[case("「`code`」", 10)]
#[case("([label](url))", 10)]
#[case("[[label](url)]", 10)]
fn wrap_preserving_code_keeps_opening_bracket_with_inline_code(
    #[case] fragment: &str,
    #[case] width: usize,
) {
    let text = format!("prefix text {fragment} suffix.");
    let lines = wrap_preserving_code(&text, width);
    for line in &lines {
        if line.contains('`') || line.contains("](") {
            assert!(
                !line.ends_with('(')
                    && !line.ends_with('[')
                    && !line.ends_with('（')
                    && !line.ends_with('「'),
                "opening bracket must stay with atomic span on line: {line:?}"
            );
        }
    }
}

#[test]
fn wrap_preserving_code_glues_punctuation_after_code() {
    let lines = wrap_preserving_code("line with `code` !", 80);
    assert_eq!(lines, vec!["line with `code`!".to_string()]);
}

#[test]
fn wrap_preserving_code_breaks_between_inline_code_spans() {
    let text = "Extensions (`.toml`, `.json`, `.json5`, `.yaml`, `.yml`).";
    // Width 35 sits between the width of the `.json` and `.json5` prefixes,
    // forcing the wrapper to decide whether it can break between separate
    // inline code spans that are spaced apart.
    let lines = wrap_preserving_code(text, 35);
    assert_eq!(
        lines,
        vec![
            "Extensions (`.toml`, `.json`,".to_string(),
            "`.json5`, `.yaml`, `.yml`).".to_string(),
        ]
    );
}

#[test]
fn wrap_preserving_code_retains_punctuation_after_separate_spans() {
    let text = "Alpha `code` `more`, trailing.";
    let lines = wrap_preserving_code(text, 18);
    assert_eq!(
        lines,
        vec!["Alpha `code`".to_string(), "`more`, trailing.".to_string(),]
    );
}

#[rstest]
#[case("alpha beta", 5, &["alpha", " beta"])]
#[case("alpha  beta", 5, &["alpha", "  beta"])]
#[case("alpha `beta`", 5, &["alpha", " `beta`"])]
fn wrap_preserving_code_preserves_carry_whitespace(
    #[case] input: &str,
    #[case] width: usize,
    #[case] expected: &[&str],
) {
    let lines = wrap_preserving_code(input, width);
    assert_eq!(
        lines,
        expected.iter().map(|&s| s.to_string()).collect::<Vec<_>>()
    );
    assert_eq!(lines.concat(), input);
}

#[rstest]
#[case("trail  ", 80, &["trail  "])]
#[case("`code span`  ", 12, &["`code span`  "])]
#[case("foo  ", 3, &["foo  "])]
#[case("x  ", 1, &["x  "])]
fn preserves_trailing_spaces(#[case] input: &str, #[case] width: usize, #[case] expected: &[&str]) {
    let out = wrap_preserving_code(input, width);
    assert_eq!(
        out,
        expected.iter().map(|&s| s.to_string()).collect::<Vec<_>>()
    );
}

#[rstest]
#[case("aaaaaaaaaaaa", 5, &["aaaaaaaaaaaa"])] // forced flush without split
#[case("abcde", 3, &["abcde"])]
#[case("`codespan`", 6, &["`codespan`"])]
fn no_split_forced_flush_no_trim(
    #[case] input: &str,
    #[case] width: usize,
    #[case] expected: &[&str],
) {
    let out = wrap_preserving_code(input, width);
    assert_eq!(
        out,
        expected.iter().map(|&s| s.to_string()).collect::<Vec<_>>()
    );
}
