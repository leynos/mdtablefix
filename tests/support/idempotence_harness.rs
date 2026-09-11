//! Shared generators and CLI harness for the idempotence property suites.
//!
//! `tests/idempotence_properties.rs` and `tests/idempotence_adjacencies.rs` are
//! separate integration-test crates, so a helper defined in one is invisible to
//! the other. The vocabulary both need lives here, pulled in with
//! `#[path = "support/idempotence_harness.rs"]`.
//!
//! Every item here is used by both binaries, because an item this module
//! defines and an including binary does not use is dead code under
//! `-D warnings`. The document-level generators stay with the properties that
//! use them for that reason: sharing them would leave most of the vocabulary
//! unreferenced in the adjacency suite.

use std::fs;

use assert_cmd::Command;
use proptest::{prelude::*, strategy::ValueTree, test_runner::TestRunner};
use tempfile::TempDir;

/// The eight transform flags the CLI exposes, in help order.
pub const FLAG_POOL: &[&str] = &[
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
pub const BREAK_SPELLINGS: &[&str] = &["---", "***", "___", "- - -", "* * *", "_ _ _"];

/// Candidate lines that must never be consumed as Setext text.
///
/// Each one is a block start under the formatter's grammar. Fence markers are
/// absent because the fence tracker skips them before the Setext pass runs;
/// they are covered by the payload table in `src/headings.rs`. HTML blocks are
/// outside the formatter's grammar and are not screened at all.
pub const NON_PARAGRAPH_STARTERS: &[&str] = &[
    "## atx heading",
    "# top level heading",
    "###### deepest heading ######",
    "---",
    "***",
    "___",
    "- - -",
    "- item",
    "1. item",
    "> quote",
    "[^1]: note",
    "[label]: https://example.com",
    "<!-- markdownlint-disable MD013 -->",
];

/// Table delimiter rows that must keep the thematic break below them.
///
/// A delimiter row is table syntax rather than paragraph text, so `---` below
/// one is a break and not an underline for it. Before the guard the pair became
/// the single line `## | --- | --- |`: the table above the row lost its
/// delimiter row, and the orphaned header row was padded differently on the
/// next pass, so the output never settled. Every spelling the table parser
/// accepts is here, including the alignment forms and the row written without a
/// leading pipe.
pub const TABLE_DELIMITER_ROWS: &[&str] = &["| --- | --- |", "|---|---|", "|:--|--:|", "--- | ---"];

/// Number of documents the non-vacuity sweeps generate.
pub const SWEEP_DOCUMENTS: usize = 64;

/// Returns the flags selected by `mask`, a bitmask over [`FLAG_POOL`].
pub fn flags_for(mask: u16) -> Vec<&'static str> {
    FLAG_POOL
        .iter()
        .enumerate()
        .filter(|(index, _)| mask & (1 << index) != 0)
        .map(|(_, flag)| *flag)
        .collect()
}

/// Formats `text` once with `flags` through the real binary, returning the bytes.
pub fn format_bytes(directory: &TempDir, text: &[u8], flags: &[&str]) -> Vec<u8> {
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
pub fn format_twice(document: &str, flags: &[&str]) -> (String, String) {
    let directory = TempDir::new().expect("creating a temporary directory");
    let once = format_bytes(&directory, document.as_bytes(), flags);
    let twice = format_bytes(&directory, &once, flags);

    (
        String::from_utf8_lossy(&once).into_owned(),
        String::from_utf8_lossy(&twice).into_owned(),
    )
}

/// Generates a lower-case word sequence of one to ten words.
pub fn prose_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec("[a-z]{2,8}", 1..=10).prop_map(|words| words.join(" "))
}

/// The defect class a generated adjacency exercises.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shape {
    /// A paragraph above its own dashes, which must become an ATX heading.
    Converting,
    /// A line that is itself a block start, which must keep the line below it.
    BlockStart,
    /// A table delimiter row, which must keep the line below it and stay a
    /// delimiter row.
    TableDelimiterRow,
}

/// A generated structural adjacency: its document (newline terminated), the
/// defect class it exercises, and the thematic break that must survive inside
/// it as a standalone line.
///
/// A tuple rather than a named struct because the two idempotence suites
/// compile this module into separate crates: the suite that only needs the
/// document never reads the other two parts, and a struct field that is never
/// read in a crate is dead code under `-D warnings`.
pub type Adjacency = (String, Shape, String);

/// Generates a line directly above a break, with an optional tail below it.
///
/// Three shapes reach the Setext pass. A paragraph above its own set of dashes
/// is the conversion the flag exists for, and here it sits immediately above a
/// thematic break, as in the reported `aa` / `-----` / `---`. A line that is
/// itself a block start above a hyphen line must keep that line: before the
/// guard, `## aa` above `---` became `## ## aa` and the break was lost. A table
/// delimiter row above a hyphen line must keep that line too, and must itself
/// stay a delimiter row: before its guard, `| --- | --- |` above `---` became
/// `## | --- | --- |` and the table above it kept drifting.
pub fn adjacency_strategy() -> impl Strategy<Value = Adjacency> {
    let fragment = prop_oneof![
        2 => (prose_strategy(), proptest::sample::select(BREAK_SPELLINGS)).prop_map(
            |(title, break_line)| {
                (
                    format!("{title}\n-----\n{break_line}"),
                    Shape::Converting,
                    break_line.to_string(),
                )
            },
        ),
        4 => proptest::sample::select(NON_PARAGRAPH_STARTERS).prop_map(|starter| {
            (
                format!("{starter}\n---"),
                Shape::BlockStart,
                "---".to_string(),
            )
        }),
        // A delimiter row alone, which the table pass has no header row to
        // attach to, and the same row below one, which is the full table the
        // reported class was found in.
        4 => proptest::sample::select(TABLE_DELIMITER_ROWS).prop_map(|row| {
            (
                format!("{row}\n---"),
                Shape::TableDelimiterRow,
                "---".to_string(),
            )
        }),
        2 => proptest::sample::select(TABLE_DELIMITER_ROWS).prop_map(|row| {
            (
                format!("| a | b |\n{row}\n---"),
                Shape::TableDelimiterRow,
                "---".to_string(),
            )
        }),
    ];

    (fragment, prop::option::of(prose_strategy())).prop_map(
        |((document, shape, break_line), tail)| {
            let document = match tail {
                Some(tail) => format!("{document}\n{tail}\n"),
                None => format!("{document}\n"),
            };

            (document, shape, break_line)
        },
    )
}

/// Samples `count` values from `strategy` with a deterministic runner.
///
/// The seed is fixed, so a coverage gap or a drifting document reproduces on
/// every run instead of appearing only on some machines.
pub fn sample<V>(strategy: &impl Strategy<Value = V>, count: usize) -> Vec<V> {
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
