//! Tests for attaching orphan language specifiers to fences.

use mdtablefix::{attach_orphan_specifiers, compress_fences};
use rstest::rstest;

#[test]
fn fixes_orphaned_specifier() {
    let input = lines_vec!["Rust", "```", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["```rust", "fn main() {}", "```"]);
}

#[rstest]
#[case(
    lines_vec!["````markdown", "Rust", "```", "fn main() {}", "```", "````"],
    lines_vec!["````markdown", "Rust", "```", "fn main() {}", "```", "````"]
)]
#[case(
    lines_vec!["````markdown", "```", "Rust", "fn main() {}", "```", "````"],
    lines_vec!["````markdown", "```", "Rust", "fn main() {}", "```", "````"]
)]
#[case(
    lines_vec!["Rust", "````", "```", "fn main() {}", "```", "````"],
    lines_vec!["````rust", "```", "fn main() {}", "```", "````"]
)]
fn handles_orphan_specifiers_around_outer_fences(
    #[case] input: Vec<String>,
    #[case] expected: Vec<String>,
) {
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, expected);
}

#[test]
fn attaches_orphan_specifier_unit() {
    let input = lines_vec!["Rust", "```", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&input);
    assert_eq!(out, lines_vec!["```rust", "fn main() {}", "```"]);
}

#[test]
fn attaches_orphan_specifier_with_blank_line_unit() {
    let input = lines_vec!["Rust", "", "```", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&input);
    assert_eq!(out, lines_vec!["```rust", "fn main() {}", "```"]);
}

#[rstest]
#[case(lines_vec!["Rust", "", "```", "fn main() {}", "```"])]
#[case(lines_vec!["Rust", "", "", "```", "fn main() {}", "```"])]
fn fixes_orphaned_specifier_with_blank_lines(#[case] input: Vec<String>) {
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["```rust", "fn main() {}", "```"]);
}

#[rstest]
#[case(lines_vec!["Rust", "", "", "```python", "print('hi')", "```"])]
#[case(lines_vec!["Rust", "", "not a fence"])]
fn leaves_orphan_specifier_and_blank_lines_unchanged_when_not_attachable(
    #[case] input: Vec<String>,
) {
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, input);
}

#[test]
fn fixes_multiple_orphaned_specifiers() {
    let input = lines_vec![
        "Rust",
        "```",
        "fn main() {}",
        "```",
        "",
        "Python",
        "```",
        "print('hi')",
        "```",
    ];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(
        out,
        lines_vec![
            "```rust",
            "fn main() {}",
            "```",
            "",
            "```python",
            "print('hi')",
            "```"
        ]
    );
}

#[test]
fn does_not_attach_non_orphan_lines_before_fences() {
    let input = lines_vec![
        "Rust code",
        "```",
        "fn main() {}",
        "```",
        "rust!",
        "```",
        "println!(\"hi\");",
        "```",
    ];
    let out = attach_orphan_specifiers(&input);
    assert_eq!(out, input);
}

#[test]
fn does_not_overwrite_existing_fence() {
    let input = lines_vec!["ruby", "```rust", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["ruby", "```rust", "fn main() {}", "```"]);
}

#[test]
fn does_not_attach_specifier_without_preceding_blank_line() {
    let input = lines_vec!["intro", "Rust", "```", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(
        out,
        lines_vec!["intro", "Rust", "```", "fn main() {}", "```"]
    );
}

#[test]
fn attaches_orphan_specifier_with_symbols() {
    let input = lines_vec!["C++", "```", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["```c++", "fn main() {}", "```"]);
}

#[test]
fn attaches_orphan_specifier_with_hyphen_and_dot() {
    let input = lines_vec!["objective-c", "```", "int main() {}", "```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["```objective-c", "int main() {}", "```"]);
}

#[test]
fn does_not_attach_specifier_with_trailing_period() {
    let input = lines_vec!["rust.", "```", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&input);
    assert_eq!(out, input);
}

#[test]
fn does_not_attach_specifier_with_trailing_question_mark() {
    let input = lines_vec!["rust?", "```", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&input);
    assert_eq!(out, input);
}

#[test]
fn attaches_orphan_specifier_preserves_indent() {
    let input = lines_vec!["  Rust", "", "  ```", "  fn main() {}", "  ```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["  ```rust", "  fn main() {}", "  ```"]);
}

#[test]
fn attaches_orphan_specifier_preserves_tab_indent() {
    let input = lines_vec!["\tRust", "", "\t```", "\tfn main() {}", "\t```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["\t```rust", "\tfn main() {}", "\t```"]);
}

#[test]
fn attaches_orphan_specifier_mixed_indent() {
    let input = lines_vec![" \tRust", "", " \t```", " \tfn main() {}", " \t```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec![" \t```rust", " \tfn main() {}", " \t```"]);
}

#[test]
fn attaches_orphan_specifier_uses_candidate_indent_when_fence_unindented() {
    let input = lines_vec!["  Rust", "", "```", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["  ```rust", "fn main() {}", "```"]);
}

#[rstest]
#[case(lines_vec!["  Rust", "", "	```", "	fn main() {}", "	```"], lines_vec!["	```rust", "	fn main() {}", "	```"])]
#[case(lines_vec!["    Rust", "", "  ```", "  fn main() {}", "  ```"], lines_vec!["    ```rust", "  fn main() {}", "  ```"])]
fn attaches_orphan_specifier_with_mismatched_indent(
    #[case] input: Vec<String>,
    #[case] expected: Vec<String>,
) {
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, expected);
}

#[test]
fn attaches_orphan_specifier_allows_spaces() {
    let input = lines_vec!["TOML, Ini", "```", "a=1", "```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["```toml,ini", "a=1", "```"]);
}
