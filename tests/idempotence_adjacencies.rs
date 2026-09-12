//! Property tests for the Setext pass's structural adjacencies (issue #474).
//!
//! A structural adjacency is a candidate line sitting directly above a thematic
//! break, where the Setext pass could read the break as an underline. Three
//! shapes reach it, and each must be a fixed point:
//!
//! - a paragraph above its own dashes, which is the conversion `--headings` exists to perform, here
//!   sitting immediately above a thematic break;
//! - a line that is itself a block start, which must keep the break below it;
//! - a table delimiter row, which must keep the break below it and must itself stay a delimiter
//!   row.
//!
//! A fourth shape chains all three in one document, because the guard for each
//! boundary has to hold where its neighbours are themselves guard cases.
//!
//! The generator and the CLI harness are shared with
//! `tests/idempotence_properties.rs` through `support/idempotence_harness.rs`.
//! This file holds the adjacency material alone, so the guard for one shape can
//! be read without the document-level generators around it: the property, the
//! coverage sweep over block-start and delimiter-row classes, and the per-shape
//! behaviour check all sit together.

use proptest::prelude::*;

#[path = "support/idempotence_harness.rs"]
mod idempotence_harness;
use idempotence_harness::{
    NON_PARAGRAPH_STARTERS,
    SWEEP_DOCUMENTS,
    Shape,
    TABLE_DELIMITER_ROWS,
    adjacency_strategy,
    flags_for,
    format_twice,
    proptest_config,
    sample,
};

/// Flag mask selecting `--headings`, the flag that enables the Setext pass.
const HEADINGS: u16 = 1 << 7;

/// Returns whether `line` is still a table delimiter row.
///
/// The shape is the one the table parser reads as the alignment row: built only
/// from pipes, colons, dashes, and spaces, carrying at least one pipe and one
/// dash. The reported class rewrote the row as `## | --- | --- |`, so the hash
/// marker alone disqualifies the line. The predicate is restated here rather
/// than shared with the other idempotence suites, because each integration test
/// file is a separate crate and a shared helper would be dead code in whichever
/// binary did not use it.
fn is_delimiter_row(line: &str) -> bool {
    line.contains('|')
        && line.contains('-')
        && line
            .chars()
            .all(|ch| matches!(ch, '|' | ':' | '-') || ch.is_whitespace())
}

proptest! {
    #![proptest_config(proptest_config())]

    /// Asserts structural adjacencies are a fixed point with `--headings` on.
    ///
    /// The flag is forced on so every case exercises the Setext pass, which the
    /// mask does not guarantee; the mask then varies the other seven flags, so
    /// the reproduction's own set (`--footnotes --code-emphasis --headings`) is
    /// among the combinations covered.
    #[test]
    fn structural_adjacencies_reach_a_fixed_point(
        adjacency in adjacency_strategy(),
        mask in 0u16..=255u16,
    ) {
        let (document, _, _) = &adjacency;
        let flags = flags_for(mask | HEADINGS);
        let (once, twice) = format_twice(document, &flags);

        prop_assert_eq!(
            &twice,
            &once,
            "an adjacency is not a fixed point for flags {:?}\ninput:\n{}\npass 1:\n{}\npass 2:\n{}",
            flags,
            document,
            once,
            twice,
        );
    }
}

/// Asserts the adjacency generator reaches every shape, and that each behaves.
///
/// A corpus of block starts alone would pass even if Setext conversion had been
/// disabled outright, and a corpus of paragraphs alone would never exercise the
/// guard. Each refusing case additionally asserts that the break below the
/// candidate survives, which is the loss the guard prevents; each delimiter-row
/// case also asserts that the row itself survives as table syntax, which is the
/// separate loss the reported class caused.
///
/// The per-shape counts are asserted rather than merely reported, so removing a
/// generator branch fails here instead of leaving the shape silently unguarded.
#[test]
fn generated_structural_adjacencies_reach_every_shape() {
    assert_eq!(
        flags_for(HEADINGS),
        vec!["--headings"],
        "the forced flag mask must select --headings",
    );

    let starters: std::collections::BTreeSet<&str> = sample(
        &proptest::sample::select(NON_PARAGRAPH_STARTERS),
        SWEEP_DOCUMENTS,
    )
    .into_iter()
    .collect();
    assert_eq!(
        starters.len(),
        NON_PARAGRAPH_STARTERS.len(),
        "the generator did not reach every block-start class",
    );

    let rows: std::collections::BTreeSet<&str> = sample(
        &proptest::sample::select(TABLE_DELIMITER_ROWS),
        SWEEP_DOCUMENTS,
    )
    .into_iter()
    .collect();
    assert_eq!(
        rows.len(),
        TABLE_DELIMITER_ROWS.len(),
        "the generator did not reach every delimiter-row spelling",
    );

    let adjacencies = sample(&adjacency_strategy(), SWEEP_DOCUMENTS);
    let mut converting = 0_usize;
    let mut refusing = 0_usize;
    let mut delimiter_rows = 0_usize;
    let mut combined = 0_usize;

    for (document, shape, break_line) in &adjacencies {
        let (once, twice) = format_twice(document, &flags_for(HEADINGS));
        assert_eq!(twice, once, "the adjacency drifted: {document:?}");

        let lines: Vec<&str> = once.lines().collect();
        assert!(
            lines.contains(&break_line.as_str()),
            "the {break_line:?} break was consumed in {document:?}: {lines:?}",
        );

        match shape {
            Shape::Converting => {
                converting += 1;
                assert!(
                    lines.iter().any(|line| line.starts_with('#')),
                    "the paragraph above a break did not convert: {lines:?}",
                );
            }
            Shape::BlockStart => refusing += 1,
            Shape::TableDelimiterRow => {
                delimiter_rows += 1;
                assert!(
                    lines.iter().any(|line| is_delimiter_row(line)),
                    "the delimiter row was consumed in {document:?}: {lines:?}",
                );
            }
            // The chain holds a converting boundary, a thematic break below it,
            // a table, and a delimiter row above the trailing break, so it
            // carries the obligations of the other shapes at once rather than
            // depending on a document that isolates one of them.
            Shape::CombinedAdjacency => {
                combined += 1;
                assert!(
                    lines.iter().any(|line| line.starts_with('#')),
                    "the paragraph above a break did not convert: {lines:?}",
                );
                assert!(
                    lines.iter().any(|line| is_delimiter_row(line)),
                    "the delimiter row was consumed in {document:?}: {lines:?}",
                );
            }
        }
    }

    assert!(
        converting > 0,
        "the generator never produced a converting adjacency",
    );
    assert!(
        refusing > 0,
        "the generator never produced a block-start adjacency",
    );
    assert!(
        delimiter_rows > 0,
        "the generator never produced a table-delimiter adjacency, so the delimiter-row guard is \
         not exercised by this sweep",
    );
    assert!(
        combined > 0,
        "the generator never produced a combined adjacency, so the shapes are only ever exercised \
         one boundary at a time",
    );
}
