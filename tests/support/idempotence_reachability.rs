//! Generator-reachability sweeps for the idempotence property suite.
//!
//! The properties themselves, the fixed-point assertions over generated
//! documents, stay in `tests/idempotence_properties.rs`. A property can only
//! fail on a shape its generator actually emits, so these sweeps sample the
//! strategies far harder than the properties do and assert that every shape
//! the properties depend on is reached: each transform flag both enabled and
//! disabled, the widened fence, ordered-marker, nested-list, table-cell, and
//! hard-break shapes, the document-level combination of them, and the line
//! termination the formatter's input contract relies on.
//!
//! The sweeps sit in a support module included by the suite rather than a
//! separate integration-test file because they draw on the same generators and
//! harness: a new test binary that did not reference every item would fail the
//! build with dead code under `-D warnings`.

use std::collections::BTreeSet;

use super::{
    idempotence_generators::{
        MAX_FENCE_RUN,
        MIN_FENCE_RUN,
        WRAP_WIDTH,
        document_strategy,
        fenced_block_strategy,
        hard_break_paragraph_strategy,
        nested_ordered_list_strategy,
        ordered_marker_strategy,
        overlong_code_span_paragraph_strategy,
        table_cell_strategy,
    },
    idempotence_harness::{FLAG_POOL, SWEEP_DOCUMENTS, TABLE_DELIMITER_ROWS, flags_for, sample},
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

/// Returns whether `line` is an ordered-list item.
///
/// A line qualifies when its first non-space character is an ASCII digit and it
/// carries the `. ` marker separator, so prose that merely opens with a digit
/// does not count.
fn is_ordered_item(line: &str) -> bool {
    let trimmed = line.trim_start();

    trimmed.chars().next().is_some_and(|ch| ch.is_ascii_digit()) && trimmed.contains(". ")
}

/// Returns whether `line` is a solid run of three to five fence markers.
fn is_fence_run(line: &str) -> bool {
    let Some(marker) = line.chars().next() else {
        return false;
    };
    if marker != '`' && marker != '~' {
        return false;
    }

    let run_len = line.chars().count();

    line.chars().all(|ch| ch == marker) && (MIN_FENCE_RUN..=MAX_FENCE_RUN).contains(&run_len)
}

/// Returns whether `line` is an ordered-list item indented by exactly three
/// spaces, the child indent `nested_ordered_list_strategy` emits.
fn is_three_space_item(line: &str) -> bool {
    line.starts_with("   ") && !line.starts_with("    ") && is_ordered_item(line)
}

/// Returns whether any line of `document` carries a code span longer than the
/// wrap width.
fn has_overlong_code_span(document: &str) -> bool {
    document
        .lines()
        .any(|line| longest_code_span_len(line) > WRAP_WIDTH)
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

/// Asserts the ordered-marker domain reaches multi-digit numbers and restarts.
#[test]
fn ordered_markers_reach_multi_digit_numbers_and_restarts() {
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
}

/// Asserts the nested-list domain reaches a parent with an indented child.
///
/// The parent is unindented and the child sits three spaces in, the content
/// column of a single-digit marker, which is still below the four-space indent
/// at which `classify_block` would read the line as indented code rather than a
/// nested list item.
#[test]
fn nested_ordered_lists_reach_a_parent_with_an_indented_child() {
    let blocks = sample(&nested_ordered_list_strategy(), DOMAIN_SWEEP);

    assert!(
        blocks.iter().any(|block| {
            let Some((parent, child)) = block.split_once('\n') else {
                return false;
            };
            let child_indent = child.len() - child.trim_start().len();

            !parent.starts_with(char::is_whitespace)
                && is_ordered_item(parent)
                && (1..=3).contains(&child_indent)
                && is_ordered_item(child)
        }),
        "no sampled block held an unindented parent above an indented child item",
    );
}

/// Asserts the table-cell domain reaches the escaped pipe and the two
/// characters the table parser and reflow helper once substituted for it.
///
/// U+001F and U+001D were the placeholders until issue #482 removed them: a
/// payload carrying either character collided with the placeholder, which is the
/// corruption that issue reports. The table pass now has to carry both through
/// as payload, and a cell only takes one by a one-in-a-million draw, so this
/// domain is what makes the case reachable at all.
#[test]
fn table_cells_reach_the_escaped_pipe_and_its_former_placeholders() {
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
    assert!(
        documents
            .iter()
            .any(|document| document.lines().any(is_fence_run)),
        "no document held a fence run, so the element strategy does not draw on it",
    );
    assert!(
        documents
            .iter()
            .any(|document| document.lines().any(is_ordered_item)),
        "no document held an ordered marker, so the element strategy does not draw on it",
    );
    assert!(
        documents
            .iter()
            .any(|document| document.lines().any(is_three_space_item)),
        "no document held a nested list item, so the element strategy does not draw on it",
    );
    assert!(
        documents
            .iter()
            .any(|document| document.contains("-----\n***\n| a | b |\n")),
        "no document held a combined adjacency, so the element strategy does not draw on it",
    );
    assert!(
        documents
            .iter()
            .any(|document| has_overlong_code_span(document)),
        "no document held an overlong code span, so the element strategy does not draw on it",
    );
}
