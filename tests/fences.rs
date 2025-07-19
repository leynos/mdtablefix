//! Tests for fence normalisation functionality.

#[macro_use]
mod prelude;
use mdtablefix::{attach_orphan_specifiers, compress_fences};

#[test]
fn compresses_backtick_fences() {
    let input = lines_vec!["````rust", "code", "````"];
    let out = compress_fences(&input);
    assert_eq!(out, lines_vec!["```rust", "code", "```"]);
}

#[test]
fn compresses_indented_backticks() {
    let input = lines_vec!["    `````foo,bar   "];
    let out = compress_fences(&input);
    assert_eq!(out, lines_vec!["    ```foo,bar"]);
}

#[test]
fn leaves_other_lines_untouched() {
    let input = lines_vec!["~~~", "``text``"];
    let out = compress_fences(&input);
    assert_eq!(out, input);
}

#[test]
fn fixes_orphaned_specifier() {
    let input = lines_vec!["Rust", "```", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["```Rust", "fn main() {}", "```"]);
}

#[test]
fn fixes_orphaned_specifier_with_blank_line() {
    let input = lines_vec!["Rust", "", "```", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["```Rust", "fn main() {}", "```"]);
}
