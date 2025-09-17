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

/// Verifies `--wrap` reflows long prose while respecting inline code spans.
#[test]
fn test_cli_wrap_reflows_long_paragraph() {
    let paragraph = concat!(
        "This deliberately long paragraph demonstrates how the ",
        "`mdtablefix --wrap` command keeps inline code spans intact even when the ",
        "surrounding prose needs to be reflowed for consistent formatting.",
    );
    let mut input = paragraph.to_owned();
    input.push(char::from(10));
    let assertion = run_cli_with_stdin(&["--wrap"], &input)
        .success();
    let output = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(
        output.ends_with(char::from(10)),
        "expected wrapped output to retain trailing newline",
    );
    assert_eq!(
        output.lines().collect::<Vec<_>>(),
        vec![
            "This deliberately long paragraph demonstrates how the `mdtablefix --wrap`",
            "command keeps inline code spans intact even when the surrounding prose needs to",
            "be reflowed for consistent formatting.",
        ],
    );
}

/// Verifies `--wrap` reflows long bulleted paragraphs with continued indentation.
#[test]
fn test_cli_wrap_reflows_bulleted_paragraph() {
    let bullet = concat!(
        "- This bulleted line is intentionally long so that mdtablefix has to wrap it ",
        "while maintaining the correct indentation for subsequent lines in the list.",
    );
    let mut input = bullet.to_owned();
    input.push(char::from(10));
    let assertion = run_cli_with_stdin(&["--wrap"], &input)
        .success();
    let output = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(
        output.ends_with(char::from(10)),
        "expected wrapped output to retain trailing newline",
    );
    assert_eq!(
        output.lines().collect::<Vec<_>>(),
        vec![
            "- This bulleted line is intentionally long so that mdtablefix has to wrap it",
            "  while maintaining the correct indentation for subsequent lines in the list.",
        ],
    );
}

/// Verifies `--wrap` reflows long numbered paragraphs with continued indentation.
#[test]
fn test_cli_wrap_reflows_numbered_paragraph() {
    let numbered = concat!(
        "1. This numbered item is intentionally long to confirm wrapping retains ",
        "numbering and indentation for subsequent lines when formatting documentation.",
    );
    let mut input = numbered.to_owned();
    input.push(char::from(10));
    let assertion = run_cli_with_stdin(&["--wrap"], &input)
        .success();
    let output = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(
        output.ends_with(char::from(10)),
        "expected wrapped output to retain trailing newline",
    );
    assert_eq!(
        output.lines().collect::<Vec<_>>(),
        vec![
            "1. This numbered item is intentionally long to confirm wrapping retains",
            "   numbering and indentation for subsequent lines when formatting documentation.",
        ],
    );
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

/// Ensures `--wrap` preserves inline code spans with embedded quotes.
#[test]
fn test_cli_wrap_preserves_inline_code_span_with_quotes() {
    let input = concat!(
        r#"- **Imperative (Avoid):** `When I type "user@example.com" into the "email"
  field and click the "submit" button` A declarative style describes the user's
  intent and the system's behaviour—the "what." It abstracts away the
  implementation details.[^18]
"#,
        r#"- **Declarative (Prefer):** `When the user logs in with valid credentials`
"#,
    );
    run_cli_with_stdin(&["--wrap"], input)
        .success()
        .stdout(input);
}

/// Ensures `--wrap` preserves emphasised step definition guidance with inline code spans.
#[test]
fn test_cli_wrap_preserves_step_definitions_guidance() {
    let input = concat!(
        r#"- **Step Definitions:** Mirror the feature file structure in your `tests/steps/` directory.
  Create a Rust module for each feature area (e.g., `tests/steps/authentication_steps.rs`,
  `tests/steps/catalog_steps.rs`). This prevents having a single, massive step definition file
  and makes it easier to find the code corresponding to a Gherkin step.
"#,
    );
    run_cli_with_stdin(&["--wrap"], input)
        .success()
        .stdout(input);
}
