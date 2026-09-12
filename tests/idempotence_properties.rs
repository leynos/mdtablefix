//! Property tests asserting the CLI formatter is a fixed point (issue #468).
//!
//! Documents are generated from the shapes that reach the formatter's defect
//! classes: thematic breaks in every spelling, prefixed lines whose first line
//! ends with an inline code span followed by a continuation or a lazy line, and
//! structural adjacencies where a candidate line sits directly above a thematic
//! break that the Setext pass could consume as an underline. Each document is
//! formatted twice through the real binary with a sampled subset of the eight
//! transform flags, and the two passes must agree byte for byte.
//!
//! The companion `idempotence.rs` test pins the issue's reproduction corpus.
//! This file covers the same ground for generated documents, so a regression in
//! a shape the corpus does not spell out still fails the suite.
//!
//! Issue #474 added a table delimiter row directly above a break as a third
//! structural adjacency. That adjacency, and the Setext pass's other two, are
//! covered in full by `tests/idempotence_adjacencies.rs`; the generator this
//! file shares with it lives in `support/idempotence_harness.rs`. Elements here
//! still draw on it, so a generated document may contain any of the three
//! shapes.
//!
//! Issue #493 widened the document domain past the three-marker, perfectly
//! balanced shapes the generators used to emit, because the longer openers,
//! mismatched interior fences, multi-digit markers, sentinel-bearing cells, and
//! hard breaks behind the recent defects were all outside it. The generators
//! moved to `support/idempotence_generators.rs` to keep both files within the
//! repository's 400-line cap; the properties and the reachability sweeps that
//! hold them to account stay here.

use std::collections::BTreeSet;

use proptest::prelude::*;

#[path = "support/idempotence_generators.rs"]
mod idempotence_generators;

#[path = "support/idempotence_harness.rs"]
mod idempotence_harness;

use idempotence_generators::{
    MAX_FENCE_RUN,
    MIN_FENCE_RUN,
    WRAP_WIDTH,
    document_strategy,
    fenced_block_strategy,
    hard_break_paragraph_strategy,
    ordered_marker_strategy,
    overlong_code_span_block_strategy,
    overlong_code_span_paragraph_strategy,
    table_cell_strategy,
};
use idempotence_harness::{
    BREAK_SPELLINGS,
    FLAG_POOL,
    SWEEP_DOCUMENTS,
    TABLE_DELIMITER_ROWS,
    flags_for,
    format_twice,
    proptest_config,
    sample,
};

/// Number of values the domain-reachability sweeps draw.
///
/// Those sweeps assert that a shape is *reachable*, so the count has to be high
/// enough for the rare corners to appear rather than merely likely. The
/// strategies are cheap to sample — no document is formatted and no process is
/// spawned — and `sample` is seeded deterministically, so the count is chosen
/// to make each assertion hold by construction rather than by luck.
const DOMAIN_SWEEP: usize = 4096;

/// Returns a fence block's marker family, opener run length, and whether the
/// document ends inside the block.
///
/// The generator writes a closer as an exact copy of the opener, so a block
/// whose last line is not its opener is one that was left unclosed.
fn fence_opener(block: &str) -> (char, usize, bool) {
    let mut lines = block.lines();
    let opener = lines.next().expect("a generated fence block has an opener");
    let marker = opener.chars().next().expect("a fence opener has a marker");
    let opener_len = opener.chars().take_while(|ch| *ch == marker).count();
    let is_unclosed = lines.next_back().is_none_or(|last| last != opener);

    (marker, opener_len, is_unclosed)
}

/// Returns whether `block` holds an interior line that is a run of the opener's
/// own marker, strictly shorter than the opener and still fence-shaped.
fn has_shorter_interior_run(block: &str) -> bool {
    let (marker, opener_len, _) = fence_opener(block);

    block.lines().skip(1).any(|line| {
        let is_solid_run = line.chars().all(|ch| ch == marker);
        let run_len = line.chars().count();

        is_solid_run && (MIN_FENCE_RUN..opener_len).contains(&run_len)
    })
}

/// Returns the length of the longest backtick-delimited run in `paragraph`.
fn longest_code_span_len(paragraph: &str) -> usize {
    paragraph
        .split('`')
        .map(str::len)
        .max()
        .expect("splitting on a delimiter always yields at least one piece")
}

proptest! {
    #![proptest_config(proptest_config())]

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
/// re-wrapping that shape must reproduce its own line breaks. Only first lines
/// that overflow the target width reach the deferral path that shape exists
/// for, so shorter ones are skipped.
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

    let class_b = sample(&overlong_code_span_block_strategy(), SWEEP_DOCUMENTS);
    let mut spans = 0_usize;
    for block in &class_b {
        let mut block_lines = block.lines();
        let Some(first_line) = block_lines.next() else {
            continue;
        };
        if !first_line.contains("(`") || block_lines.next().is_none() || first_line.len() <= 80 {
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

/// Asserts the fence domain reaches long openers, shorter interior runs, and
/// unclosed blocks.
///
/// Issue #480's counterexample needs all three at once: an opener longer than
/// the run inside it, an interior run that a compressed opener would treat as a
/// closer, and a block the document ends inside. Balanced three-marker fences
/// alone cannot reach any of them, so the shape would stay untested however
/// many cases the property ran.
#[test]
fn fence_domain_reaches_long_openers_shorter_interior_runs_and_unclosed_blocks() {
    let blocks = sample(&fenced_block_strategy(), DOMAIN_SWEEP);
    let openers: Vec<(char, usize, bool)> =
        blocks.iter().map(|block| fence_opener(block)).collect();

    let markers: BTreeSet<char> = openers.iter().map(|(marker, ..)| *marker).collect();
    assert_eq!(
        markers,
        BTreeSet::from(['`', '~']),
        "the fence generator did not reach both marker families",
    );

    let lengths: BTreeSet<usize> = openers.iter().map(|(_, len, _)| *len).collect();
    assert_eq!(
        lengths,
        (MIN_FENCE_RUN..=MAX_FENCE_RUN).collect(),
        "the fence generator did not reach every opener length",
    );

    assert!(
        openers.iter().any(|(_, _, is_unclosed)| *is_unclosed),
        "the fence generator never left a block unclosed, so the unmatched path is untested",
    );
    assert!(
        blocks.iter().any(|block| has_shorter_interior_run(block)),
        "no generated block put a fence-shaped run of the opener's own marker inside it",
    );
}

/// Asserts the ordered-marker domain reaches multi-digit numbers, restarts, and
/// nested depths.
#[test]
fn ordered_markers_reach_multi_digit_numbers_restarts_and_nesting() {
    let markers = sample(&ordered_marker_strategy(), DOMAIN_SWEEP);
    let numbers: Vec<u32> = markers
        .iter()
        .map(|marker| {
            marker
                .trim()
                .trim_end_matches('.')
                .parse()
                .expect("a generated ordered marker ends with a decimal number and a dot")
        })
        .collect();

    assert!(
        numbers.iter().any(|number| *number >= 100),
        "no ordered marker reached three digits",
    );
    assert!(numbers.contains(&1), "no ordered marker restarted at 1");
    assert!(
        markers.iter().any(|marker| marker.starts_with("    ")),
        "no ordered marker reached a nested depth",
    );
}

/// Asserts the table-cell domain reaches the escaped pipe and the two in-band
/// sentinels the table parser and reflow helper substitute for it.
///
/// A cell carrying U+001F or U+001D literally collides with a placeholder,
/// which is the corruption issue #482 reports. The in-crate generator filters
/// both characters out, so this domain is the only one that can reach it.
#[test]
fn table_cells_reach_the_parser_sentinels() {
    let cells = sample(&table_cell_strategy(), DOMAIN_SWEEP);

    assert!(
        cells.iter().any(|cell| cell.contains("\\|")),
        "no generated cell carried an escaped pipe",
    );
    assert!(
        cells.iter().any(|cell| cell.contains('\u{1f}')),
        "no generated cell carried U+001F",
    );
    assert!(
        cells.iter().any(|cell| cell.contains('\u{1d}')),
        "no generated cell carried U+001D",
    );
}

/// Asserts the paragraph domain reaches both hard-break markers and wraps
/// around a code span longer than the target width.
#[test]
fn paragraphs_reach_hard_breaks_and_overlong_code_spans() {
    let hard_breaks = sample(&hard_break_paragraph_strategy(), DOMAIN_SWEEP);
    assert!(
        hard_breaks
            .iter()
            .any(|paragraph| paragraph.lines().any(|line| line.ends_with("  "))),
        "no generated paragraph carried a two-space hard break",
    );
    assert!(
        hard_breaks
            .iter()
            .any(|paragraph| paragraph.lines().any(|line| line.ends_with('\\'))),
        "no generated paragraph carried a backslash hard break",
    );

    let spans = sample(&overlong_code_span_paragraph_strategy(), DOMAIN_SWEEP);
    let shortest = spans
        .iter()
        .map(|paragraph| longest_code_span_len(paragraph))
        .min()
        .expect("the sweep produced no paragraphs");
    assert!(
        shortest > WRAP_WIDTH,
        "a generated code span was {shortest} characters, not longer than the {WRAP_WIDTH}-column \
         wrap width",
    );
}

/// Asserts generated documents carry the shapes the widened domain added.
///
/// The per-strategy sweeps above show each generator reaches its shapes; this
/// one shows the element generator actually draws on them, so an arm dropped
/// from `element_strategy` fails here rather than leaving the property vacuous.
#[test]
fn generated_documents_carry_the_widened_shapes() {
    let documents = sample(&document_strategy(), SWEEP_DOCUMENTS);

    assert!(
        documents.iter().any(|document| TABLE_DELIMITER_ROWS
            .iter()
            .any(|row| document.contains(row))),
        "no generated document contained a table",
    );
    assert!(
        documents.iter().any(|document| document
            .lines()
            .any(|line| line.ends_with("  ") || line.ends_with('\\'))),
        "no generated document contained a hard break",
    );
}
