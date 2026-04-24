//! CLI regression tests for fence normalization edge cases.

#[macro_use]
mod prelude;
use prelude::*;

#[test]
fn test_cli_fences_preserves_nested_backtick_block() {
    let input = concat!(
        "````markdown\n",
        "```rust\n",
        "fn main() {}\n",
        "```\n",
        "````\n",
    );

    run_cli_with_stdin(&["--fences"], input)
        .success()
        .stdout(input);
}

#[test]
fn test_cli_fences_preserves_nested_backticks_inside_tilde_block() {
    let input = concat!(
        "~~~~markdown\n",
        "```rust\n",
        "fn main() {}\n",
        "```\n",
        "~~~~\n",
    );

    run_cli_with_stdin(&["--fences"], input)
        .success()
        .stdout(input);
}

#[test]
fn test_cli_fences_compresses_outer_backticks_while_preserving_inner_tildes() {
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

    run_cli_with_stdin(&["--fences"], input)
        .success()
        .stdout(expected);
}
