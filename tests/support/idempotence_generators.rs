//! Document-level generators for the CLI fixed-point property suite.
//!
//! `tests/idempotence_properties.rs` is the only binary that draws on these, so
//! they live here rather than in `idempotence_harness.rs`: that module is
//! compiled into both idempotence binaries, and a generator one of them never
//! calls would be dead code under `-D warnings`. Splitting the document
//! vocabulary out also keeps each file within the repository's 400-line cap.
//!
//! The domain is deliberately wider than the shapes the formatter is known to
//! handle. Fence openers run from three to five markers in either family, so an
//! interior line can carry a shorter run of the opener's own marker; ordered
//! markers reach three digits and nested depths; table cells hold any character
//! a source line can carry, sentinels included; and paragraphs end lines with
//! hard breaks or wrap around inline code spans longer than the wrap width.

use proptest::prelude::*;

use super::idempotence_harness::{
    BREAK_SPELLINGS,
    TABLE_DELIMITER_ROWS,
    adjacency_strategy,
    prose_strategy,
};

/// The wrap width the CLI applies, in columns.
pub const WRAP_WIDTH: usize = 80;

/// The shortest marker run that is still a fence delimiter.
pub const MIN_FENCE_RUN: usize = 3;

/// The longest marker run a generated fence delimiter uses.
pub const MAX_FENCE_RUN: usize = 5;

/// Markdown hard-break markers: two trailing spaces, and a trailing backslash.
const HARD_BREAK_MARKERS: &[&str] = &["  ", "\\"];

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

/// Generates an ordered-list marker, sometimes nested.
///
/// The number reaches three digits so the renumber pass sees the multi-digit
/// markers and restarts the single-digit `1.` arm never produced: consecutive
/// elements of one document draw their own numbers, so a document can hold a
/// list that jumps or restarts. An indent of one or two two-space steps keeps
/// the marker a list item rather than an indented code block, matching
/// `list_prefix_strategy` in `tests/wrap_leading_spaces.rs`.
pub fn ordered_marker_strategy() -> impl Strategy<Value = String> {
    (1_u32..=999, 0_usize..=2)
        .prop_map(|(number, depth)| format!("{}{number}. ", "  ".repeat(depth)))
}

/// Generates a prefixed line: a bullet, task, ordered, quote, or footnote line.
fn prefixed_line_strategy() -> impl Strategy<Value = String> {
    let marker = prop_oneof![
        3 => Just("- ".to_string()),
        1 => Just("- [ ] ".to_string()),
        2 => Just("1. ".to_string()),
        2 => ordered_marker_strategy(),
        2 => Just("> ".to_string()),
        1 => Just("[^1]: ".to_string()),
        1 => Just("  - ".to_string()),
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

/// Generates a prefixed block whose first line overflows the target width and
/// ends with a parenthesised inline code span, plus a continuation line.
///
/// This is the class B shape: the first line spills past the wrap width, so the
/// block is deferred and must reflow with the continuation below it. The prose
/// is grown until the line exceeds the width, because a short line never
/// reaches the deferral path.
pub fn overlong_code_span_block_strategy() -> impl Strategy<Value = String> {
    (
        prose_strategy(),
        code_span_strategy(),
        continuation_strategy(),
    )
        .prop_map(|(prose, span, continuation)| {
            let mut line = format!("- {prose}");
            while line.len() + span.len() + 3 <= WRAP_WIDTH {
                line.push_str(" and more prose");
            }

            format!("{line} ({span})\n{continuation}")
        })
}

/// A fenced block's marker family and opening run length.
#[derive(Clone, Copy, Debug)]
struct FenceShape {
    marker: char,
    opener_len: usize,
    /// The length of a strictly shorter run of the same marker that an interior
    /// line can carry, or `None` for an opener too short to admit one.
    interior_len: Option<usize>,
}

/// Generates a fence shape: either marker family, an opener of three to five
/// markers, and a shorter same-marker run for the interior when one fits.
fn fence_shape_strategy() -> impl Strategy<Value = FenceShape> {
    let opener = (
        prop_oneof![Just('`'), Just('~')],
        MIN_FENCE_RUN..=MAX_FENCE_RUN,
    );
    opener.prop_flat_map(|(marker, opener_len)| {
        // An interior run has to clear `MIN_FENCE_RUN` to be a fence-shaped
        // line at all, and be shorter than the opener, so the shortest opener
        // admits none. `option::of` over an empty range would panic, hence the
        // explicit degenerate case.
        let interior_len = if opener_len > MIN_FENCE_RUN {
            prop::option::of(MIN_FENCE_RUN..opener_len).boxed()
        } else {
            Just(None).boxed()
        };

        interior_len.prop_map(move |interior_len| FenceShape {
            marker,
            opener_len,
            interior_len,
        })
    })
}

/// Generates one interior line of a fenced block.
///
/// A run of the opener's own marker that is shorter than the opener is legal
/// Markdown inside a longer fence, and is the shape issue #480 turns on:
/// compressing the opener to three markers lets that interior line close the
/// block and reinterprets the rest of the payload as prose. A run of the other
/// marker family is generated too, which the fence tracker also treats as
/// conflicting interior content.
fn fence_interior_strategy(shape: FenceShape) -> impl Strategy<Value = String> {
    let other_marker = if shape.marker == '`' { '~' } else { '`' };
    let same_marker = shape.marker;
    let shorter_run = shape.interior_len.map_or_else(String::new, |len| {
        std::iter::repeat_n(same_marker, len).collect()
    });

    prop_oneof![
        4 => prose_strategy(),
        1 => Just(String::new()),
        3 => (MIN_FENCE_RUN..=MAX_FENCE_RUN)
            .prop_map(move |len| std::iter::repeat_n(other_marker, len).collect()),
        3 => Just(shorter_run),
    ]
}

/// Generates a fenced code block: either marker family, an opener of three to
/// five markers, and either a matching closer or none at all.
///
/// An unmatched block leaves the document inside the fence, which is the path
/// `flush_unmatched_block` serves. Both paths have to keep the opener long
/// enough that an interior shorter run stays literal content.
pub fn fenced_block_strategy() -> impl Strategy<Value = String> {
    (fence_shape_strategy(), any::<bool>()).prop_flat_map(|(shape, is_unmatched)| {
        let interior = prop::collection::vec(fence_interior_strategy(shape), 1..=4);

        interior.prop_map(move |interior| {
            let opener: String = std::iter::repeat_n(shape.marker, shape.opener_len).collect();
            let closer = (!is_unmatched).then(|| opener.clone());

            std::iter::once(opener)
                .chain(interior)
                .chain(closer)
                .collect::<Vec<_>>()
                .join("\n")
        })
    })
}

/// Generates a character a table cell can hold.
///
/// The line terminators are folded to spaces rather than filtered out, so no
/// drawn value is ever rejected and shrinking is not perturbed. Half the draws
/// come from the ASCII range and half from the whole `char` range: U+001F and
/// U+001D are single code points in a space of over a million, so drawing
/// uniformly over that space would leave them effectively unreachable, and with
/// them the corruption issue #482 reports.
fn single_line_character_strategy() -> impl Strategy<Value = char> {
    prop_oneof![
        1 => proptest::char::range('\u{0}', '\u{7f}'),
        1 => any::<char>(),
    ]
    .prop_map(|character| match character {
        '\r' | '\n' => ' ',
        other => other,
    })
}

/// Generates one table cell's text.
///
/// Pipes are escaped as `\|` so the row stays a single row. That escape is
/// deliberate: `split_cells` substitutes U+001F for `\|` and the reflow helper
/// marks a leading empty cell with U+001D, so a payload carrying either
/// character literally collides with a placeholder. The in-crate generator in
/// `src/reflow/tests.rs` filters both characters out, so nothing there reaches
/// it; this one reaches them directly and reaches `\|` as well.
pub fn table_cell_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(single_line_character_strategy(), 0..=8).prop_map(|characters| {
        characters
            .into_iter()
            .map(|character| match character {
                '|' => "\\|".to_string(),
                other => other.to_string(),
            })
            .collect()
    })
}

/// Generates a two-column table row bounded by pipes.
fn table_row_strategy() -> impl Strategy<Value = String> {
    (table_cell_strategy(), table_cell_strategy())
        .prop_map(|(left, right)| format!("| {left} | {right} |"))
}

/// Generates a GFM table: a header row, one of the accepted delimiter rows, and
/// at least one body row.
///
/// The column count is fixed at two because every spelling in
/// [`TABLE_DELIMITER_ROWS`] has two columns, and a header whose width disagrees
/// with its delimiter row is not a table the parser recognises.
fn table_strategy() -> impl Strategy<Value = String> {
    (
        proptest::sample::select(TABLE_DELIMITER_ROWS),
        table_row_strategy(),
        prop::collection::vec(table_row_strategy(), 1..=3),
    )
        .prop_map(|(delimiter, header, body)| {
            std::iter::once(header)
                .chain(std::iter::once(delimiter.to_string()))
                .chain(body)
                .collect::<Vec<_>>()
                .join("\n")
        })
}

/// Generates a paragraph whose interior lines end with a hard break.
///
/// `trailing_hard_break_marker_len` reads two trailing spaces or an odd run of
/// trailing backslashes as a hard break, and either is a reason to keep a line
/// break rather than reflow across it.
pub fn hard_break_paragraph_strategy() -> impl Strategy<Value = String> {
    (
        prose_strategy(),
        prop::collection::vec(
            (
                prose_strategy(),
                proptest::sample::select(HARD_BREAK_MARKERS),
            ),
            1..=3,
        ),
    )
        .prop_map(|(head, tail)| {
            std::iter::once(head)
                .chain(
                    tail.into_iter()
                        .map(|(prose, marker)| format!("{prose}{marker}")),
                )
                .collect::<Vec<_>>()
                .join("\n")
        })
}

/// Generates a paragraph holding an inline code span longer than the wrap
/// width.
///
/// A span that cannot fit between two breaks has no legal place to split, so
/// the wrap has to choose between leaving the line overlong and breaking inside
/// the span, and has to make the same choice on both passes.
pub fn overlong_code_span_paragraph_strategy() -> impl Strategy<Value = String> {
    (
        prose_strategy(),
        prose_strategy(),
        (WRAP_WIDTH + 1)..=(WRAP_WIDTH * 2),
    )
        .prop_map(|(head, tail, span_len)| {
            let span = "x".repeat(span_len);
            format!("{head} `{span}` {tail}")
        })
}

/// Generates one element of a document; elements are joined by newlines.
fn element_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => prose_strategy(),
        4 => prefixed_block_strategy(),
        2 => proptest::sample::select(BREAK_SPELLINGS).prop_map(str::to_string),
        2 => adjacency_strategy()
            .prop_map(|(document, _, _)| document.trim_end_matches('\n').to_string()),
        2 => fenced_block_strategy(),
        2 => table_strategy(),
        2 => hard_break_paragraph_strategy(),
        2 => overlong_code_span_paragraph_strategy(),
        1 => prose_strategy().prop_map(|title| format!("{title}\n-----")),
        1 => Just("[1] and text... here".to_string()),
        1 => Just("**bold**`code`".to_string()),
        1 => Just(String::new()),
    ]
}

/// Generates a whole document with a trailing newline.
pub fn document_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(element_strategy(), 1..=10)
        .prop_map(|elements| elements.join("\n") + "\n")
}
