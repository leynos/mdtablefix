//! CLI wrapping option tests.
//!
//! Validates that the `--wrap` command-line flag correctly triggers text
//! wrapping behaviour when processing Markdown content through the `mdtablefix`
//! binary.

use super::*;

#[test]
fn test_cli_wrap_option() {
    let input = "This line is deliberately made much longer than eighty columns so that the \
                 wrapping algorithm is forced to insert a soft line-break somewhere in the middle \
                 of the paragraph when the --wrap flag is supplied.";
    let assertion = run_cli_with_stdin(&["--wrap"], &format!("{input}\n"))
        .success();
    let text = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(text.lines().count() > 1, "expected wrapped output on multiple lines");
    assert!(text.lines().all(|l| l.len() <= 80));
}

/// Ensures `--wrap` preserves an explicit language specifier on fences.
#[test]
fn test_cli_wrap_preserves_language() {
    let input = "```rust\nfn main() {}\n```\n";
    run_cli_with_stdin(&["--wrap"], input)
        .success()
        .stdout(input);
}

/// Accepts an optional space between the fence marker and language.
#[test]
fn test_cli_wrap_preserves_language_with_space() {
    let input = "``` rust\nfn main() {}\n```\n";
    run_cli_with_stdin(&["--wrap"], input)
        .success()
        .stdout(input);
}

/// Validates handling of opening fences without language specifiers.
#[test]
fn test_cli_wrap_preserves_plain_fence() {
    let input = "```\ncode\n```\n";
    run_cli_with_stdin(&["--wrap"], input)
        .success()
        .stdout(input);
}

/// Ensures `--wrap` preserves indented fenced code blocks.
#[test]
fn test_cli_wrap_preserves_indented_fence() {
    let input = "    ```rust\n    fn main() {}\n    ```\n";
    run_cli_with_stdin(&["--wrap"], input)
        .success()
        .stdout(input);
}

/// Ensures `--wrap` preserves tildes as fence markers with language.
#[test]
fn test_cli_wrap_preserves_tilde_fence_with_language() {
    let input = "~~~python\nprint('Hello, world!')\n~~~\n";
    run_cli_with_stdin(&["--wrap"], input)
        .success()
        .stdout(input);
}

/// Ensures `--wrap` preserves tildes as fence markers without language.
#[test]
fn test_cli_wrap_preserves_tilde_fence_without_language() {
    let input = "~~~\nno language here\n~~~\n";
    run_cli_with_stdin(&["--wrap"], input)
        .success()
        .stdout(input);
}

/// Opening with four backticks should ignore inner triple backticks.
#[test]
fn test_cli_wrap_preserves_four_backticks_and_ignores_inner_triple() {
    let input = "````rust\n```\nfn main() {}\n````\n";
    run_cli_with_stdin(&["--wrap"], input)
        .success()
        .stdout(input);
}

/// Retains extended info strings including attributes and options.
#[test]
fn test_cli_wrap_preserves_extended_info_string() {
    let input = "``` rust linenums {style=monokai}\ncode\n```\n";
    run_cli_with_stdin(&["--wrap"], input)
        .success()
        .stdout(input);
}

/// Accepts four or more tildes as fence markers.
#[test]
fn test_cli_wrap_preserves_tilde_with_four_markers() {
    let input = "~~~~python\nprint('hi')\n~~~~\n";
    run_cli_with_stdin(&["--wrap"], input)
        .success()
        .stdout(input);
}
