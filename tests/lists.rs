//! Integration tests for list renumbering and counters.

use mdtablefix::{lists::pop_counters_upto, renumber_lists};

#[macro_use]
mod common;
use assert_cmd::Command;

#[test]
fn pop_counters_removes_deeper_levels() {
    let mut counters = vec![(0usize, 1usize), (4, 2), (8, 3)];
    pop_counters_upto(&mut counters, 4);
    assert_eq!(counters, vec![(0, 1)]);
}

#[test]
fn pop_counters_no_change_when_indent_deeper() {
    let mut counters = vec![(0usize, 1usize), (4, 2)];
    pop_counters_upto(&mut counters, 6);
    assert_eq!(counters, vec![(0, 1), (4, 2)]);
}

#[test]
fn restart_after_lower_paragraph() {
    let input = lines_vec!("1. One", "", "Paragraph", "3. Next");
    let expected = lines_vec!("1. One", "", "Paragraph", "1. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn no_restart_without_blank() {
    let input = lines_vec!("1. One", "Paragraph", "3. Next");
    let expected = lines_vec!("1. One", "Paragraph", "2. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn no_restart_for_indented_paragraph() {
    let input = lines_vec!("1. One", "", "  Indented", "3. Next");
    let expected = lines_vec!("1. One", "", "  Indented", "2. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn no_restart_for_non_plain_line() {
    let input = lines_vec!("1. One", "", "# Heading", "3. Next");
    let expected = lines_vec!("1. One", "", "# Heading", "2. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn restart_after_nested_paragraph() {
    let input = lines_vec!("1. One", "    1. Sub", "", "Paragraph", "3. Next");
    let expected = lines_vec!("1. One", "    1. Sub", "", "Paragraph", "1. Next");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn restart_after_formatting_paragraph() {
    let input = lines_vec!("1. Start", "", "**Bold intro**", "", "4. Next");
    let expected = lines_vec!("1. Start", "", "**Bold intro**", "", "1. Next");
    assert_eq!(renumber_lists(&input), expected);
}
#[test]
fn test_renumber_basic() {
    let input = lines_vec!["1. first", "2. second", "7. third"];
    let expected = lines_vec!["1. first", "2. second", "3. third"];
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_renumber_with_fence() {
    let input = lines_vec!["1. item", "```", "code", "```", "9. next"];
    let expected = lines_vec!["1. item", "```", "code", "```", "2. next"];
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_cli_renumber_option() {
    let output = Command::cargo_bin("mdtablefix")
        .unwrap()
        .arg("--renumber")
        .write_stdin("1. a\n4. b\n")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(text, "1. a\n2. b\n");
}

#[test]
fn test_renumber_nested_lists() {
    let input = lines_vec![
        "1. first",
        "    1. sub first",
        "    3. sub second",
        "2. second",
    ];

    let expected = lines_vec![
        "1. first",
        "    1. sub first",
        "    2. sub second",
        "2. second",
    ];

    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_renumber_tabs_in_indent() {
    let input = lines_vec!["1. first", "\t1. sub first", "\t5. sub second", "2. second"];

    let expected = lines_vec!["1. first", "\t1. sub first", "\t2. sub second", "2. second"];

    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_renumber_mult_paragraph_items() {
    let input = lines_vec!["1. first", "", "    still first paragraph", "", "2. second"];

    let expected = lines_vec!["1. first", "", "    still first paragraph", "", "2. second"];

    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_renumber_table_in_list() {
    let input = lines_vec!["1. first", "    | A | B |", "    | 1 | 2 |", "5. second"];

    let expected = lines_vec!["1. first", "    | A | B |", "    | 1 | 2 |", "2. second"];

    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_renumber_restart_after_paragraph() {
    let input: Vec<String> = include_lines!("data/renumber_paragraph_restart_input.txt");
    let expected: Vec<String> = include_lines!("data/renumber_paragraph_restart_expected.txt");
    assert_eq!(renumber_lists(&input), expected);
}

#[test]
fn test_renumber_restart_after_formatting_paragraph() {
    let input: Vec<String> = include_str!("data/renumber_formatting_paragraph_input.txt")
        .lines()
        .map(str::to_string)
        .collect();
    let expected: Vec<String> = include_str!("data/renumber_formatting_paragraph_expected.txt")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(renumber_lists(&input), expected);
}
