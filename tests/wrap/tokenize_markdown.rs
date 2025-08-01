//! Tests for the tokenize_markdown helper.

use mdtablefix::wrap::{self, Token};

#[test]
fn unclosed_fence_yields_fence_tokens() {
    let lines = vec!["```rust", "let x = 42;", "fn foo() {}"];
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
fn malformed_fence_is_text() {
    let source = "``~~\ncode\n``~~";
    let tokens = wrap::tokenize_markdown(source);
    assert_eq!(
        tokens,
        vec![
            Token::Text("``~~"),
            Token::Newline,
            Token::Text("code"),
            Token::Newline,
            Token::Text("``~~"),
        ]
    );
}

#[test]
fn incorrect_fence_length_is_text() {
    let source = "````\ncode\n````";
    let tokens = wrap::tokenize_markdown(source);
    assert_eq!(
        tokens,
        vec![
            Token::Text("````"),
            Token::Newline,
            Token::Text("code"),
            Token::Newline,
            Token::Text("````"),
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

