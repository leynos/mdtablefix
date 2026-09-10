//! Property tests asserting the CLI formatter is a fixed point (issue #468).
//!
//! Documents are generated from the shapes that reach the formatter's two
//! defect classes: thematic breaks in every spelling, and prefixed lines whose
//! first line ends with an inline code span followed by a continuation or a
//! lazy line. Each document is formatted twice through the real binary with a
//! sampled subset of the eight transform flags, and the two passes must agree
//! byte for byte.
//!
//! The companion `idempotence.rs` test pins the issue's reproduction corpus.
//! This file covers the same ground for generated documents, so a regression in
//! a shape the corpus does not spell out still fails the suite.

use std::fs;

use assert_cmd::Command;
use proptest::{
    prelude::*,
    strategy::ValueTree,
    test_runner::{Config as ProptestConfig, TestRunner},
};
use tempfile::TempDir;

/// The eight transform flags the CLI exposes, in help order.
const FLAG_POOL: &[&str] = &[
    "--wrap",
    "--renumber",
    "--breaks",
    "--ellipsis",
    "--fences",
    "--footnotes",
    "--code-emphasis",
    "--headings",
];

/// Thematic break spellings; each must survive as a standalone line.
const BREAK_SPELLINGS: &[&str] = &["---", "***", "___", "- - -", "* * *", "_ _ _"];

/// Number of documents the non-vacuity sweep generates.
const SWEEP_DOCUMENTS: usize = 64;

/// Returns the flags selected by `mask`, a bitmask over [`FLAG_POOL`].
fn flags_for(mask: u16) -> Vec<&'static str> {
    FLAG_POOL
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, flag)| *flag)
        .collect()
}

/// Formats `text` once with `flags` through the real binary, returning the bytes.
fn format_bytes(directory: &TempDir, text: &[u8], flags: &[&str]) -> Vec<u8> {
    let path = directory.path().join("case.md");
    fs::write(&path, text).expect("writing the case file");

    Command::cargo_bin("mdtablefix")
        .expect("locating the mdtablefix binary")
        .args(flags)
        .arg("--in-place")
        .arg(&path)
        .assert()
        .success();

    fs::read(&path).expect("reading the formatted case")
}

/// Formats `document` twice with `flags`, returning both passes as strings.
fn format_twice(document: &str, flags: &[&str]) -> (String, String) {
    let directory = TempDir::new().expect("creating a temporary directory");
    let once = format_bytes(&directory, document.as_bytes(), flags);
    let twice = format_bytes(&directory, &once, flags);

    (
        String::from_utf8_lossy(&once).into_owned(),
        String::from_utf8_lossy(&twice).into_owned(),
    )
}

/// Generates a lower-case word sequence of one to ten words.
fn prose_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec("[a-z]{2,8}", 1..=10).prop_map(|words| words.join(" "))
}

/// Generates an inline code span shaped like a file path.
fn code_span_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec("[a-z]{2,6}", 1..=3)
        .prop_map(|segments| format!("`{}`", segments.join("/")))
}

/// Generates the tail that follows a prefix marker.
///
/// A parenthesised code span is the class B shape: the wrap's line breaking
/// depends on that trailing token, so it decides whether the block reflows with
/// the lines below it.
fn tail_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        2 => Just(String::new()),
        3 => prose_strategy().prop_map(|prose| format!(" {prose}")),
        3 => prose_strategy().prop_map(|prose| format!(" ({prose})")),
        3 => code_span_strategy().prop_map(|span| format!(" ({span})")),
    ]
}

/// Generates a prefixed line: a bullet, task, ordered, quote, or footnote line.
fn prefixed_line_strategy() -> impl Strategy<Value = String> {
    let marker = prop_oneof![
        3 => Just("- "),
        1 => Just("- [ ] "),
        2 => Just("1. "),
        2 => Just("> "),
        1 => Just("[^1]: "),
        1 => Just("  - "),
    ];

    (marker, prose_strategy(), tail_strategy())
        .prop_map(|(marker, prose, tail)| format!("{marker}{prose}{tail}"))
}

/// Generates the line below a prefixed line: indented, lazy, code, or a quote.
fn continuation_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => prose_strategy().prop_map(|prose| format!("  {prose}")),
        2 => prose_strategy(),
        1 => prose_strategy().prop_map(|prose| format!("    {prose}")),
        1 => prose_strategy().prop_map(|prose| format!("> {prose}")),
        1 => Just(String::new()),
    ]
}

/// Generates a prefixed block, sometimes with a continuation line below it.
fn prefixed_block_strategy() -> impl Strategy<Value = String> {
    (
        prefixed_line_strategy(),
        prop::option::of(continuation_strategy()),
    )
        .prop_map(|(line, continuation)| match continuation {
            Some(continuation) => format!("{line}\n{continuation}"),
            None => line,
        })
}

/// Generates a fenced code block with either fence spelling.
fn fenced_block_strategy() -> impl Strategy<Value = String> {
    let fence = prop_oneof![Just("```"), Just("~~~")];

    (fence, prose_strategy()).prop_map(|(fence, body)| format!("{fence}\n{body}\n{fence}"))
}

/// Generates one element of a document; elements are joined by newlines.
fn element_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => prose_strategy(),
        4 => prefixed_block_strategy(),
        2 => proptest::sample::select(BREAK_SPELLINGS).prop_map(str::to_string),
        2 => fenced_block_strategy(),
        1 => prose_strategy().prop_map(|title| format!("{title}\n-----")),
        1 => Just("[1] and text... here".to_string()),
        1 => Just("**bold**`code`".to_string()),
        1 => Just(String::new()),
    ]
}

/// Generates a whole document with a trailing newline.
fn document_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(element_strategy(), 1..=10)
        .prop_map(|elements| elements.join("\n") + "\n")
}

/// Samples `count` values from `strategy` with a deterministic runner.
///
/// The seed is fixed, so a coverage gap or a drifting document reproduces on
/// every run instead of appearing only on some machines.
fn sample<V>(strategy: &impl Strategy<Value = V>, count: usize) -> Vec<V> {
    let mut runner = TestRunner::deterministic();
    let mut values = Vec::with_capacity(count);

    for _ in 0..count {
        let value = strategy
            .new_tree(&mut runner)
            .expect("building a generated value")
            .current();
        values.push(value);
    }

    values
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Asserts the CLI formatter is a fixed point for generated documents.
    ///
    /// The mask samples the eight-flag powerset, so the property covers
    /// `--wrap` alone, every other flag alone, the `make fmt` flag set, and
    /// everything in between.
    #[test]
    fn cli_formatting_reaches_a_fixed_point(
        document in document_strategy(),
        mask in 0u16..=255u16,
    ) {
        let flags = flags_for(mask);
        let (once, twice) = format_twice(&document, &flags);

        prop_assert_eq!(
            &twice,
            &once,
            "formatting is not a fixed point for flags {:?}\ninput:\n{}\npass 1:\n{}\npass 2:\n{}",
            flags,
            document,
            once,
            twice,
        );
    }
}

/// Asserts the sweep samples every flag both enabled and disabled.
///
/// A flag that is never enabled would make the sweep vacuous for that
/// transform, and one that is never disabled would hide interactions between
/// the flags.
#[test]
fn generated_corpus_reaches_every_flag() {
    let masks = sample(&(0u16..=255u16), SWEEP_DOCUMENTS);
    let mut seen_enabled = std::collections::BTreeSet::new();
    for mask in &masks {
        for flag in flags_for(*mask) {
            seen_enabled.insert(flag);
        }
    }

    let missing: Vec<_> = FLAG_POOL
        .iter()
        .filter(|flag| !seen_enabled.contains(**flag))
        .collect();
    assert!(
        missing.is_empty(),
        "the sweep never enabled {missing:?} across {SWEEP_DOCUMENTS} samples",
    );

    for (index, flag) in FLAG_POOL.iter().enumerate() {
        let bit = 1 << index;
        assert!(
            masks.iter().any(|mask| mask & bit == 0),
            "the sweep never disabled {flag}",
        );
    }
}

/// Asserts the generator produces both changed and unchanged documents.
///
/// The fixed-point property is only meaningful over documents the formatter
/// actually rewrites: a corpus the formatter already leaves alone would satisfy
/// the property without exercising it.
#[test]
fn generated_corpus_produces_changed_and_unchanged_documents() {
    let documents = sample(&document_strategy(), SWEEP_DOCUMENTS);
    let mut changed = 0_usize;
    let mut unchanged = 0_usize;

    for document in &documents {
        let (once, twice) = format_twice(document, &flags_for(u16::MAX));
        assert_eq!(twice, once, "generated document drifted: {document:?}");
        if once == *document {
            unchanged += 1;
        } else {
            changed += 1;
        }
    }

    assert!(changed > 0, "no generated document was reformatted");
    assert!(unchanged > 0, "no generated document was already formatted");
}

/// Asserts the generated documents reach both defect classes.
///
/// Class A is a thematic break in any spelling: each one the generator emits
/// must survive a wrap as a standalone line, which is the property the
/// normalised 70-underscore break lost. Class B is a prefixed line whose first
/// line ends with a parenthesised inline code span followed by a continuation;
/// re-wrapping that shape must reproduce its own line breaks.
#[test]
fn generated_corpus_reaches_both_defect_classes() {
    let breaks = sample(&proptest::sample::select(BREAK_SPELLINGS), SWEEP_DOCUMENTS);
    let mut covered = std::collections::BTreeSet::new();
    for break_line in &breaks {
        let input: Vec<String> = ["alpha", break_line, "beta"]
            .iter()
            .map(|line| (*line).to_string())
            .collect();
        let once = mdtablefix::wrap::wrap_text(&input, 80);
        assert!(
            once.iter().any(|line| line == break_line),
            "wrap absorbed the {break_line:?} break: {once:?}",
        );
        covered.insert(*break_line);
    }
    assert_eq!(
        covered.len(),
        BREAK_SPELLINGS.len(),
        "the generator did not reach every break spelling",
    );

    let class_b = sample(&prefixed_block_strategy(), SWEEP_DOCUMENTS);
    let mut spans = 0_usize;
    for block in &class_b {
        let mut block_lines = block.lines();
        let Some(first_line) = block_lines.next() else {
            continue;
        };
        if !first_line.contains("(`") || block_lines.next().is_none() {
            continue;
        }

        spans += 1;
        let document = format!("{block}\n");
        let (once, twice) = format_twice(&document, &flags_for(1));
        assert_eq!(
            twice, once,
            "the class B shape is not a fixed point: {document:?}",
        );
    }
    assert!(
        spans > 0,
        "the generator never produced a prefixed line with a code span and a continuation",
    );
}

/// Asserts generated documents are newline terminated without carriage returns.
#[test]
fn generated_documents_are_line_terminated() {
    for document in sample(&document_strategy(), 16) {
        assert!(
            document.ends_with('\n'),
            "generated document is not newline terminated: {document:?}",
        );
        assert!(
            !document.contains('\r'),
            "generated document contains a carriage return: {document:?}",
        );
    }
}
