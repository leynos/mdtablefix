//! Tests for the `tokenize_markdown` helper.

use mdtablefix::wrap::{self, Token};
use rstest::rstest;

#[test]
fn unclosed_fence_yields_fence_tokens() {
    let lines = ["```rust", "let x = 42;", "fn foo() {}"];
    let joined = lines.join("\n");
    let tokens = wrap::tokenize_markdown(&joined);
    assert_eq!(
        tokens,
        vec![
            Token::Fence("```rust"),
            Token::Newline,
            Token::Fence("let x = 42;"),
            Token::Newline,
            Token::Fence("fn foo() {}"),
        ]
    );
}

#[test]
fn closed_fence_yields_fence_tokens_until_closer() {
    let source = "```sql\nSELECT ...;\n{...}\n```\nafter `code`";
    let tokens = wrap::tokenize_markdown(source);
    assert_eq!(
        tokens,
        vec![
            Token::Fence("```sql"),
            Token::Newline,
            Token::Fence("SELECT ...;"),
            Token::Newline,
            Token::Fence("{...}"),
            Token::Newline,
            Token::Fence("```"),
            Token::Newline,
            Token::Text("after "),
            Token::Code {
                raw: "`code`",
                fence: "`",
                code: "code",
            },
        ]
    );
}

#[test]
fn nested_shorter_fence_remains_literal_until_outer_closer() {
    let source = "````markdown\n```json\n{...}\n```\n````\nafter";
    let tokens = wrap::tokenize_markdown(source);
    assert_eq!(
        tokens,
        vec![
            Token::Fence("````markdown"),
            Token::Newline,
            Token::Fence("```json"),
            Token::Newline,
            Token::Fence("{...}"),
            Token::Newline,
            Token::Fence("```"),
            Token::Newline,
            Token::Fence("````"),
            Token::Newline,
            Token::Text("after"),
        ]
    );
}

#[test]
fn malformed_fence_is_text() {
    let source = "``~~\ncode\n``~~";
    let tokens = wrap::tokenize_markdown(source);
    assert_eq!(
        tokens,
        vec![
            Token::Text("``"),
            Token::Text("~~"),
            Token::Newline,
            Token::Text("code"),
            Token::Newline,
            Token::Text("``"),
            Token::Text("~~"),
        ]
    );
}
#[test]
fn unmatched_inline_code_is_text() {
    let source = "bad `code span";
    let tokens = wrap::tokenize_markdown(source);
    assert_eq!(
        tokens,
        vec![
            Token::Text("bad "),
            Token::Text("`"),
            Token::Text("code span"),
        ]
    );
}

#[test]
fn multiple_unmatched_backticks_are_text() {
    let source = "``bad code";
    let tokens = wrap::tokenize_markdown(source);
    assert_eq!(tokens, vec![Token::Text("``"), Token::Text("bad code"),]);
}

#[test]
fn multibyte_characters_round_trip() {
    let source = "ßß `λ` fin";
    let tokens = wrap::tokenize_markdown(source);
    assert_eq!(
        tokens,
        vec![
            Token::Text("ßß "),
            Token::Code {
                raw: "`λ`",
                fence: "`",
                code: "λ",
            },
            Token::Text(" fin"),
        ]
    );
}

#[rstest]
#[case(
    "`VarGuard`s",
    Token::Code {
        raw: "`VarGuard`s",
        fence: "`",
        code: "VarGuard",
    }
)]
#[case(
    "`class`'s",
    Token::Code {
        raw: "`class`'s",
        fence: "`",
        code: "class",
    }
)]
#[case(
    "`fetch`ed",
    Token::Code {
        raw: "`fetch`ed",
        fence: "`",
        code: "fetch",
    }
)]
#[case(
    "`run`ning",
    Token::Code {
        raw: "`run`ning",
        fence: "`",
        code: "run",
    }
)]
#[case(
    "`code`-style",
    Token::Code {
        raw: "`code`-style",
        fence: "`",
        code: "code",
    }
)]
fn inline_code_with_suffix_emits_single_token(#[case] source: &str, #[case] expected: Token<'_>) {
    let tokens = wrap::tokenize_markdown(source);
    assert_eq!(tokens, vec![expected]);
}

#[test]
fn inline_code_followed_by_whitespace_does_not_absorb_suffix() {
    let source = "`code` word";
    let tokens = wrap::tokenize_markdown(source);
    assert_eq!(
        tokens,
        vec![
            Token::Code {
                raw: "`code`",
                fence: "`",
                code: "code",
            },
            Token::Text(" word"),
        ]
    );
}
