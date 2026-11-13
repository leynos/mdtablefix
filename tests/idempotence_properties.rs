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
//! mismatched interior fences, multi-digit markers, control-character cells, and
//! hard breaks behind the recent defects were all outside it. The generators
//! moved to `support/idempotence_generators.rs`, which no other binary draws on;
//! the properties stay here, and the reachability sweeps that hold the
//! generators to account live in `support/idempotence_reachability.rs`.
//!
//! Issue #504 added the bracket reference seam: a paragraph whose prose and
//! inline code span fill all but one of the first line's columns, with a bare
//! `[1]` on the line below. The wrap boundary falls between the two, which is
//! where the opener used to be left behind. Unlike the earlier shapes the seam
//! needs an exact width, so `bracket_reference_seam_strategy` grows its prose to
//! that width instead of emitting whatever the prose strategy returns.

use mdtablefix::process::WRAP_COLS;
use proptest::prelude::*;

#[path = "support/idempotence_generators.rs"]
mod idempotence_generators;

#[path = "support/idempotence_harness.rs"]
mod idempotence_harness;
#[path = "support/layout_normalization.rs"]
mod layout_normalization;

// The reachability sweeps share this binary, so the generator items they import stay used.
#[path = "support/idempotence_reachability.rs"]
mod idempotence_reachability;

use idempotence_generators::{document_strategy, overlong_code_span_block_strategy};
use idempotence_harness::{
    BREAK_SPELLINGS,
    SWEEP_DOCUMENTS,
    flags_for,
    format_twice,
    proptest_config,
    sample,
};

/// Display columns a bracket reference seam head fills.
///
/// One column short of the wrap width, so a reference can never join the first
/// line: the space that would precede it already fills the line.
const BRACKET_SEAM_HEAD_WIDTH: usize = WRAP_COLS - 1;

/// Generates the issue #504 seam: an inline code span filling the first line,
/// with a bare bracket reference on the line below.
///
/// The two lines are one paragraph, so the wrap boundary falls between the code
/// span and the reference. Before the fix the opener stayed at the end of the
/// first line and the next pass rejoined the halves as `[ 1]`. Prose lengths
/// that do not reach the target width never put the boundary there, so the
/// strategy grows every head to it.
fn bracket_reference_seam_strategy() -> impl Strategy<Value = String> {
    (proptest::collection::vec("[a-z]{2,6}", 1..=6), "[0-9]{1,2}").prop_map(|(words, digits)| {
        let mut head = format!("{} **bold**`code`", words.join(" "));
        while head.len() + 5 <= BRACKET_SEAM_HEAD_WIDTH {
            head.push_str(" aaaa");
        }
        if head.len() < BRACKET_SEAM_HEAD_WIDTH {
            head.push(' ');
            head.push_str(&"a".repeat(BRACKET_SEAM_HEAD_WIDTH - head.len()));
        }

        format!("{head}\n[{digits}]")
    })
}

/// Generates a valid table with an emphasis-wrapped inline code span.
///
/// This is the shape whose repair shortens a table cell. The marker and prose
/// vary independently so the property covers both emphasis spellings and
/// realistic cell widths without generating malformed marker sequences that
/// the code-emphasis transform intentionally repairs differently on a later
/// standalone invocation.
fn code_emphasis_table_strategy() -> impl Strategy<Value = String> {
    let words = || proptest::collection::vec("[a-z]{2,8}", 1..=3).prop_map(|words| words.join(" "));

    (
        words(),
        words(),
        words(),
        prop_oneof![Just("*"), Just("_")],
        words(),
        words(),
    )
        .prop_map(|(header, row, prefix, marker, code, suffix)| {
            format!(
                "| {header} | Notes |\n| --- | ----- |\n| {row} | {prefix} \
                 {marker}`{code}`{marker} {suffix} |\n"
            )
        })
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

    /// Asserts code-emphasis table repair reaches a fixed point in one pass.
    #[test]
    fn code_emphasis_table_repair_reaches_a_fixed_point(
        document in code_emphasis_table_strategy(),
        wrap in any::<bool>(),
        ellipsis in any::<bool>(),
    ) {
        let mut flags = vec!["--code-emphasis"];
        if wrap {
            flags.push("--wrap");
        }
        if ellipsis {
            flags.push("--ellipsis");
        }
        let (once, twice) = format_twice(&document, &flags);

        prop_assert_eq!(
            &twice,
            &once,
            "code-emphasis table formatting is not a fixed point for flags {:?}\\ninput:\\n{}\\npass 1:\\n{}\\npass 2:\\n{}",
            flags,
            document,
            once,
            twice,
        );
    }
}

/// Keeps an unspaced emphasis-to-code boundary from gaining a space on reread.
#[test]
fn adjacent_emphasis_and_code_reaches_a_fixed_point() {
    let document = concat!(
        "aaaaaaaa aaaaaa aaa aaaa aaaa aa dhaogoog axa gr\n",
        "vi lg\n",
        "nwqwaa jrc kojnx **bold**`code` aaaa aaaa aaaa aaaa aaaa aaaa aaaa aaaa aaaa aa\n",
        "[1]\n",
        "gcxjrit hcaolazw bto yzilku esvuo milveecc xkdfafl\n",
    );
    let flags = flags_for(71);
    let (once, twice) = format_twice(document, &flags);

    assert_eq!(
        once,
        concat!(
            "aaaaaaaa aaaaaa aaa aaaa aaaa aa dhaogoog axa gr vi lg nwqwaa jrc kojnx\n",
            "**bold**`code` aaaa aaaa aaaa aaaa aaaa aaaa aaaa aaaa aaaa aa [1] gcxjrit\n",
            "hcaolazw bto yzilku esvuo milveecc xkdfafl\n",
        ),
    );
    assert_eq!(twice, once, "the exact mask-71 case is not a fixed point");
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

/// Asserts the generated corpus reaches the issue #504 wrapping seam.
///
/// The seam is a paragraph whose prose and inline code span fill all but one of
/// the first line's columns, with the bracket reference on the next source
/// line, so the wrap boundary falls between the code span and the reference.
/// Only heads that reach the target width put the boundary there, so the count
/// is asserted nonzero rather than assuming every sample qualifies: a generator
/// that stopped growing the prose would leave this shape uncovered.
#[test]
fn generated_corpus_reaches_the_bracket_reference_seam() {
    let seams = sample(&bracket_reference_seam_strategy(), SWEEP_DOCUMENTS);
    let mut reached = 0_usize;

    for seam in &seams {
        let mut lines = seam.lines();
        let Some(head) = lines.next() else {
            continue;
        };
        let Some(reference) = lines.next() else {
            continue;
        };
        if head.len() != BRACKET_SEAM_HEAD_WIDTH || !reference.starts_with('[') {
            continue;
        }

        reached += 1;
        let document = format!("{seam}\n");
        let (once, twice) = format_twice(&document, &flags_for(1));
        assert_eq!(
            twice, once,
            "the bracket reference seam is not a fixed point: {document:?}",
        );
        assert!(
            once.lines().any(|line| line.trim() == reference),
            "expected {reference:?} to survive as a line of {once:?}",
        );
        assert!(
            once.lines().all(|line| !line.ends_with('[')),
            "opening bracket was stranded in {once:?}",
        );
    }

    assert!(
        reached > 0,
        "the generator never produced a paragraph that fills the line before a bracket reference",
    );
}
