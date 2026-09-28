//! Regression tests for issue #561.
//!
//! Invariant: `--wrap` never inserts a break where the source had no
//! whitespace, and a break it makes never leaves trailing spaces.
//!
//! Markdown renders a soft line break as a space, so a break placed between two
//! characters that touch in the source inserts a space into the rendered text,
//! and two or more spaces left at a line end render as a hard break. The cases
//! here pin every reproduction shape from the issue: `(` before a reference
//! link, `/` between two code spans, `-[` inside a word, `**` around a code
//! span, the mirror `][4]).`, and a break at a run of two spaces. The property
//! checks the general form over generated paragraphs whose words are glued by
//! punctuation and Markdown syntax: the whitespace-separated words of the
//! output are exactly those of the input, which is the rendered-content oracle
//! for this defect (no word may gain a boundary). It also checks that no line
//! the wrapper broke ends in a space, and that a second pass changes nothing.

use assert_cmd::Command;
use mdtablefix::{process::WRAP_COLS, wrap::wrap_text};
use proptest::prelude::*;
use rstest::rstest;

const PAD: &str = "The word word word word word word word word word word word word see";

/// Wraps one paragraph at the production width.
fn wrap(paragraph: &str) -> Vec<String> { wrap_text(&[paragraph.to_owned()], WRAP_COLS) }

/// Returns the whitespace-separated words of some lines.
fn words(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .flat_map(|line| line.split_whitespace())
        .map(str::to_owned)
        .collect()
}

/// Scenario: a paragraph whose 80-column boundary falls inside a run of
/// characters that touch in the source.
///
/// Invariant: the run stays on one line, so the rendered text gains no space.
#[rstest]
#[case::opening_parenthesis_before_reference_link("([Python Packaging][4])")]
#[case::slash_between_code_spans("`Mutex`/`MutexGuard`")]
#[case::bracket_inside_a_word("(Alice)-[r]->(Bob)")]
#[case::strong_emphasis_around_code("**`Node.isConnected`**")]
fn a_touching_run_is_never_split(#[case] run: &str) {
    let paragraph = format!("{PAD} {run} tail words here.");
    let lines = wrap(&paragraph);
    assert!(
        lines.iter().any(|line| line.contains(run)),
        "{run:?} was split: {lines:#?}"
    );
    assert_eq!(words(&lines), words(&[paragraph]));
}

/// Scenario: a closing bracket followed by punctuation sits on the boundary.
///
/// Invariant: `][4]).` stays attached to the link text before it.
#[test]
fn a_closing_bracket_keeps_its_punctuation() {
    let paragraph = format!(
        "{} {PAD} [Python Packaging][4]). tail words here.",
        "x".repeat(66)
    );
    let lines = wrap(&paragraph);
    assert!(
        lines.iter().all(|line| !line.trim_start().starts_with(')')),
        "a line starts with the closing punctuation: {lines:#?}"
    );
    assert_eq!(words(&lines), words(&[paragraph]));
}

/// Scenario: the wrapper breaks at a run of two spaces inside a list item.
///
/// Invariant: the broken line keeps no trailing spaces, so no hard break
/// appears where the source had none.
#[test]
fn a_break_at_a_double_space_leaves_no_hard_break() {
    let item = concat!(
        "- Reliable clipboard, ctrl+enter queue batching, selection auto-scroll  ",
        "(a6d912d2) (@7jrxt42BxFZo4iAnN4CX)",
    );
    let lines = wrap(item);
    assert!(lines.len() > 1, "expected a wrap: {lines:#?}");
    assert!(
        lines[..lines.len() - 1]
            .iter()
            .all(|line| !line.ends_with(' ')),
        "a wrapped line ends in a space: {lines:#?}"
    );
}

/// Scenario: a source hard break (two trailing spaces) ends the paragraph's
/// first line.
///
/// Invariant: the hard break survives, since the source wrote it.
#[test]
fn a_source_hard_break_survives() {
    let lines = wrap_text(
        &["First line.  ".to_owned(), "Second line.".to_owned()],
        WRAP_COLS,
    );
    assert_eq!(
        lines,
        vec!["First line.  ".to_owned(), "Second line.".to_owned()]
    );
}

/// Runs the real binary with `--wrap` over `input` and returns stdout.
fn wrap_cli(input: &str) -> String {
    let output = Command::cargo_bin("mdtablefix")
        .expect("the mdtablefix binary builds")
        .arg("--wrap")
        .write_stdin(input)
        .output()
        .expect("mdtablefix runs");
    assert!(output.status.success(), "mdtablefix failed: {output:?}");
    String::from_utf8(output.stdout).expect("mdtablefix writes UTF-8")
}

/// Scenario: the command line formats a document holding every reported shape,
/// a break at a run of two spaces and a source hard break.
///
/// Invariant: the snapshot pins the exact output: touching runs whole, no
/// trailing spaces on wrapper-broken lines, and the source hard break kept.
/// A second run over the output changes nothing.
#[test]
fn the_command_line_keeps_touching_runs_whole() {
    let input = [
        format!("{PAD} ([Python Packaging][4]) tail words here."),
        String::new(),
        format!("{PAD} `Mutex`/`MutexGuard` tail words here."),
        String::new(),
        format!("{PAD} (Alice)-[r]->(Bob) tail words here."),
        String::new(),
        format!("{PAD} **`Node.isConnected`** tail words here."),
        String::new(),
        concat!(
            "- Reliable clipboard, ctrl+enter queue batching, selection auto-scroll  ",
            "(a6d912d2) (@7jrxt42BxFZo4iAnN4CX)",
        )
        .to_owned(),
        String::new(),
        "First line.  ".to_owned(),
        "Second line.".to_owned(),
        String::new(),
    ]
    .join("\n");
    let output = wrap_cli(&input);
    insta::assert_snapshot!(output);
    assert_eq!(wrap_cli(&output), output, "a second run changed the output");
}

/// Returns one glue string that joins two words with no whitespace.
fn glue() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("/"),
        Just("-"),
        Just("("),
        Just(")"),
        Just("["),
        Just("]"),
        Just("**"),
        Just("`/`"),
        Just(")."),
        Just("]("),
    ]
}

/// Returns one word: plain, a code span, or a reference link.
fn word() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z]{1,9}",
        "[a-z]{1,7}".prop_map(|text| format!("`{text}`")),
        ("[a-z]{1,6}", 1_u8..9).prop_map(|(text, n)| format!("[{text}][{n}]")),
    ]
}

/// Returns a paragraph whose tokens are glued by syntax or separated by spaces.
fn paragraph() -> impl Strategy<Value = String> {
    prop::collection::vec((word(), prop::option::of(glue())), 4..40).prop_map(|parts| {
        let mut text = String::new();
        for (index, (word, joint)) in parts.into_iter().enumerate() {
            if index > 0 {
                text.push(' ');
            }
            text.push_str(&word);
            if let Some(joint) = joint {
                text.push_str(joint);
            }
        }
        text
    })
}

proptest! {
    /// Wrapping at any width keeps every source word whole, leaves no trailing
    /// space on a line it broke, and is a fixed point.
    #[test]
    fn wrapping_only_breaks_at_source_whitespace(text in paragraph(), width in 20_usize..100) {
        let lines = wrap_text(std::slice::from_ref(&text), width);
        prop_assert_eq!(words(&lines), words(&[text]));
        for line in &lines[..lines.len().saturating_sub(1)] {
            prop_assert!(!line.ends_with(' '), "trailing space on {:?}", line);
        }
        prop_assert_eq!(wrap_text(&lines, width), lines);
    }
}
