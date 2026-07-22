//! Tests for fence normalization functionality.

#[macro_use]
#[path = "common/mod.rs"]
mod common;
use mdtablefix::{attach_orphan_specifiers, compress_fences};
use rstest::rstest;

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
fn compresses_tilde_fences() {
    let input = lines_vec!["~~~~rust", "code", "~~~~"];
    let out = compress_fences(&input);
    assert_eq!(out, lines_vec!["```rust", "code", "```"]);
}

#[rstest]
#[case(
    lines_vec!["````markdown", "```rust", "fn main() {}", "```", "````"],
    lines_vec!["````markdown", "```rust", "fn main() {}", "```", "````"]
)]
#[case(
    lines_vec!["~~~~markdown", "~~~rust", "fn main() {}", "~~~", "~~~~"],
    lines_vec!["~~~~markdown", "~~~rust", "fn main() {}", "~~~", "~~~~"]
)]
#[case(
    lines_vec!["~~~markdown", "```rust", "fn main() {}", "```", "~~~"],
    lines_vec!["~~~markdown", "```rust", "fn main() {}", "```", "~~~"]
)]
#[case(
    lines_vec!["````markdown", "~~~rust", "fn main() {}", "~~~", "````"],
    lines_vec!["```markdown", "~~~rust", "fn main() {}", "~~~", "```"]
)]
#[case(
    lines_vec!["``` rust", "~~~example", "code", "~~~", "```"],
    lines_vec!["``` rust", "~~~example", "code", "~~~", "```"]
)]
#[case(
    lines_vec!["```` rust", "code", "````"],
    lines_vec!["```` rust", "code", "````"]
)]
fn preserves_nested_or_spaced_fence_blocks(
    #[case] input: Vec<String>,
    #[case] expected: Vec<String>,
) {
    let out = compress_fences(&input);
    assert_eq!(out, expected);
}

#[test]
fn does_not_compress_mixed_fences() {
    // Each block is unclosed because the trailing delimiter uses a different
    // marker character, so only the opening delimiter is normalized and the
    // interior line is preserved verbatim.
    let input = lines_vec!["~~~rust", "code", "```"];
    let out = compress_fences(&input);
    assert_eq!(out, lines_vec!["```rust", "code", "```"]);

    let input2 = lines_vec!["```rust", "code", "~~~"];
    let out2 = compress_fences(&input2);
    assert_eq!(out2, lines_vec!["```rust", "code", "~~~"]);
}

#[test]
fn leaves_other_lines_untouched() {
    let input = lines_vec!["~~", "``text``"];
    let out = compress_fences(&input);
    assert_eq!(out, input);
}

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

#[rstest]
#[case("````null", "````")]
#[case("````NULL", "````")]
#[case("````Null", "````")]
#[case("````null  ", "````")]
#[case("````   ", "````")]
#[case("````NULL  ", "````")]
#[case("````Null  ", "````")]
#[case("~~~~null", "~~~~")]
#[case("~~~~NULL", "~~~~")]
#[case("~~~~Null", "~~~~")]
#[case("~~~~null  ", "~~~~")]
#[case("~~~~   ", "~~~~")]
#[case("~~~~NULL  ", "~~~~")]
#[case("~~~~Null  ", "~~~~")]
fn compresses_null_language_to_empty(#[case] open: &str, #[case] close: &str) {
    let input = lines_vec![open, "code", close];
    let out = compress_fences(&input);
    assert_eq!(out, lines_vec!["```", "code", "```"]);
}

#[rstest]
#[case("```null")]
#[case("```NULL")]
#[case("```Null")]
#[case("```null  ")]
#[case("```NULL  ")]
#[case("```Null  ")]
#[case("~~~~null")]
#[case("~~~~NULL")]
#[case("~~~~Null")]
#[case("~~~~null  ")]
#[case("~~~~NULL  ")]
#[case("~~~~Null  ")]
fn attaches_orphan_specifier_when_null_language(#[case] fence: &str) {
    let input = lines_vec!["Rust", fence, "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["```rust", "fn main() {}", "```"]);
}

#[test]
fn attaches_orphan_specifier_null_language_without_compression() {
    let input = lines_vec!["Rust", "```null", "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&input);
    assert_eq!(out, lines_vec!["```rust", "fn main() {}", "```"]);
}

#[rstest]
#[case("```   ")]
#[case("~~~~   ")]
fn attaches_orphan_specifier_whitespace_language(#[case] fence: &str) {
    let input = lines_vec!["Rust", fence, "fn main() {}", "```"];
    let out = attach_orphan_specifiers(&compress_fences(&input));
    assert_eq!(out, lines_vec!["```rust", "fn main() {}", "```"]);
}

#[test]
fn compresses_matched_fence_reusing_cached_opening_and_closing_rewrites() {
    // Both delimiters are rewritable and are emitted from the per-line cache,
    // while the interior content lines pass through untouched.
    let input = lines_vec!["`````rust", "let x = 1;", "let y = 2;", "`````"];
    let out = compress_fences(&input);
    assert_eq!(
        out,
        lines_vec!["```rust", "let x = 1;", "let y = 2;", "```"]
    );
}

#[test]
fn unclosed_fence_rewrites_only_the_opening_delimiter() {
    // No closing delimiter matches the six-backtick opener, so the block is
    // emitted through the unmatched fallback. Only the opening delimiter is
    // normalized; the interior fence-like lines are literal content of the
    // unclosed fence and are preserved verbatim.
    let input = lines_vec!["``````rust", "````js", "~~~"];
    let out = compress_fences(&input);
    assert_eq!(out, lines_vec!["```rust", "````js", "~~~"]);
}

#[rstest]
#[case(lines_vec!["> ```rust", "> code", "> ```"])]
#[case(lines_vec!["> > ````toml", "> > data", "> > ````"])]
#[case(lines_vec![">```", ">code", ">```"])]
fn preserves_quoted_and_nested_fences(#[case] input: Vec<String>) {
    // Blockquoted delimiters are not normalization-compatible, so quoted and
    // nested fenced blocks round-trip unchanged while remaining correctly opened
    // and closed at their blockquote depth.
    let out = compress_fences(&input);
    assert_eq!(out, input);
}

#[test]
fn compresses_top_level_fence_after_quoted_block() {
    // A quoted block passes through untouched; the following unquoted block is
    // compressed, proving depth transitions do not leak between the two.
    let input = lines_vec!["> ````rust", "> code", "> ````", "````rust", "top", "````",];
    let out = compress_fences(&input);
    assert_eq!(
        out,
        lines_vec!["> ````rust", "> code", "> ````", "```rust", "top", "```",]
    );
}
