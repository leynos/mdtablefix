//! Thematic-break pass-through tests for `wrap_text`.
//!
//! A thematic break must survive wrapping byte-for-byte on a line of its own.
//! Otherwise a break normalised by `--breaks` is absorbed into the adjacent
//! paragraph by a later `--wrap` pass, and `mdtablefix --check` can never agree
//! with `mdtablefix --in-place`.

use rstest::rstest;

use crate::wrap::wrap_text;

/// The underscore run `--breaks` emits in place of every other break form.
const NORMALISED_BREAK: &str =
    "______________________________________________________________________";

fn lines(text: &str) -> Vec<String> { text.lines().map(str::to_string).collect() }

#[rstest]
#[case("---")]
#[case("***")]
#[case("___")]
#[case("- - -")]
#[case("* * *")]
#[case("_ _ _")]
#[case("  ---")]
fn wrap_text_keeps_thematic_break_on_its_own_line(#[case] break_line: &str) {
    let input = lines(&format!("alpha\n{break_line}\nbeta"));

    let wrapped = wrap_text(&input, 80);

    assert_eq!(
        wrapped,
        vec![
            "alpha".to_string(),
            break_line.to_string(),
            "beta".to_string(),
        ]
    );
}

#[test]
fn wrap_text_keeps_normalised_break_on_its_own_line() {
    let input = lines(&format!("prose words here\n{NORMALISED_BREAK}\nmore prose"));

    let wrapped = wrap_text(&input, 80);

    assert_eq!(
        wrapped,
        vec![
            "prose words here".to_string(),
            NORMALISED_BREAK.to_string(),
            "more prose".to_string(),
        ]
    );
}

#[test]
fn wrap_text_keeps_break_after_an_open_code_span() {
    // The unclosed span defers the bullet into a pending prefix; the break
    // that follows must still terminate the block rather than be matched as a
    // bullet marker by the continuation handler.
    let input = lines("- item with `open span\n- - -\nbody");

    let wrapped = wrap_text(&input, 80);

    assert!(
        wrapped.contains(&"- - -".to_string()),
        "spaced break must survive a pending prefixed span: {wrapped:?}"
    );
}
