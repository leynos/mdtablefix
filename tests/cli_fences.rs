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
