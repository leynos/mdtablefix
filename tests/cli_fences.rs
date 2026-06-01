//! CLI regression tests for fence normalization edge cases.

#[path = "support/cli_stdin.rs"]
mod cli_stdin;
use cli_stdin::run_cli_with_stdin;

#[test]
fn test_cli_fences_preserves_nested_backtick_block() -> Result<(), Box<dyn std::error::Error>> {
    let input = concat!(
        "````markdown\n",
        "```rust\n",
        "fn main() {}\n",
        "```\n",
        "````\n",
    );

    let assertion = run_cli_with_stdin(&["--fences"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

#[test]
fn test_cli_fences_preserves_nested_backticks_inside_tilde_block()
-> Result<(), Box<dyn std::error::Error>> {
    let input = concat!(
        "~~~~markdown\n",
        "```rust\n",
        "fn main() {}\n",
        "```\n",
        "~~~~\n",
    );

    let assertion = run_cli_with_stdin(&["--fences"], input)?;
    assertion.success().stdout(input);
    Ok(())
}

#[test]
fn test_cli_fences_compresses_outer_backticks_while_preserving_inner_tildes()
-> Result<(), Box<dyn std::error::Error>> {
    let input = concat!(
        "````markdown\n",
        "~~~rust\n",
        "fn main() {}\n",
        "~~~\n",
        "````\n",
    );
    let expected = concat!(
        "```markdown\n",
        "~~~rust\n",
        "fn main() {}\n",
        "~~~\n",
        "```\n",
    );

    let assertion = run_cli_with_stdin(&["--fences"], input)?;
    assertion.success().stdout(expected);
    Ok(())
}
