//! CLI tests for preserving fences and inline code during wrapping.

use super::run_cli_with_stdin;

/// Ensures `--wrap` preserves an explicit language specifier on fences.
#[test]
fn test_cli_wrap_preserves_language() -> Result<(), Box<dyn std::error::Error>> {
    let input = "```rust\nfn main() {}\n```\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Accepts an optional space between the fence marker and language.
#[test]
fn test_cli_wrap_preserves_language_with_space() -> Result<(), Box<dyn std::error::Error>> {
    let input = "``` rust\nfn main() {}\n```\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Validates handling of opening fences without language specifiers.
#[test]
fn test_cli_wrap_preserves_plain_fence() -> Result<(), Box<dyn std::error::Error>> {
    let input = "```\ncode\n```\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Ensures `--wrap` preserves indented fenced code blocks.
#[test]
fn test_cli_wrap_preserves_indented_fence() -> Result<(), Box<dyn std::error::Error>> {
    let input = "    ```rust\n    fn main() {}\n    ```\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Ensures `--wrap` preserves tildes as fence markers with language.
#[test]
fn test_cli_wrap_preserves_tilde_fence_with_language() -> Result<(), Box<dyn std::error::Error>> {
    let input = "~~~python\nprint('Hello, world!')\n~~~\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Ensures `--wrap` preserves tildes as fence markers without language.
#[test]
fn test_cli_wrap_preserves_tilde_fence_without_language() -> Result<(), Box<dyn std::error::Error>>
{
    let input = "~~~\nno language here\n~~~\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Opening with four backticks should ignore inner triple backticks.
#[test]
fn test_cli_wrap_preserves_four_backticks_and_ignores_inner_triple()
-> Result<(), Box<dyn std::error::Error>> {
    let input = "````rust\n```\nfn main() {}\n````\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Retains extended info strings including attributes and options.
#[test]
fn test_cli_wrap_preserves_extended_info_string() -> Result<(), Box<dyn std::error::Error>> {
    let input = "``` rust linenums {style=monokai}\ncode\n```\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Accepts four or more tildes as fence markers.
#[test]
fn test_cli_wrap_preserves_tilde_with_four_markers() -> Result<(), Box<dyn std::error::Error>> {
    let input = "~~~~python\nprint('hi')\n~~~~\n";
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

/// Ensures `--wrap` preserves inline code spans with embedded quotes.
#[test]
fn test_cli_wrap_preserves_inline_code_span_with_quotes() -> Result<(), Box<dyn std::error::Error>>
{
    let input = concat!(
        r#"- **Imperative (Avoid):** `When I type "user@example.com" into the "email"
  field and click the "submit" button` A declarative style describes the user's
  intent and the system's behaviour—the "what." It abstracts away the
  implementation details.[^18]
"#,
        r#"- **Declarative (Prefer):** `When the user logs in with valid credentials`
"#,
    );
    let assertion = run_cli_with_stdin(&["--wrap"], input)?;
    let success = assertion.success();
    let output = String::from_utf8_lossy(&success.get_output().stdout);
    assert!(
        output.contains("user@example.com"),
        "email literal must remain intact after wrapping"
    );
    assert!(
        output.contains("field and click the \"submit\" button"),
        "submit step text must remain intact after wrapping"
    );
    assert!(
        output.contains("`When the user logs in with valid credentials`"),
        "second inline code span must remain intact"
    );
    Ok(())
}
