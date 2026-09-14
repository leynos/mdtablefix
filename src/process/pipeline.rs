//! Named pipeline stages that prepare a document for the layout passes.
//!
//! [`super::process_stream_inner`] calls these in the order the stages must
//! run, so the orchestration reads as a sequence rather than as one long body.
//! They live in their own module so the parent module stays within the
//! repository size limit, as [`super::buffer`] does.

use super::{
    Options,
    buffer::{ProcessBuffer, TableSubstitutions},
};
use crate::{
    fences::{attach_orphan_specifiers, compress_fences},
    footnotes::{convert_inline_footnotes_with_setext, renumber_footnote_labels},
    wrap::FenceTracker,
};

/// Normalizes code fences when `opts.fences` is set.
///
/// Compression runs first, and orphan specifiers are attached to the compressed
/// blocks. A document that does not request fence handling is copied instead,
/// so every caller owns its input.
pub(super) fn normalize_fences(lines: &[String], opts: Options) -> Vec<String> {
    if opts.fences {
        let compressed = compress_fences(lines);
        return attach_orphan_specifiers(&compressed);
    }

    lines.to_vec()
}

/// Rewrites the footnote reference markers that the later passes measure.
///
/// Footnote references rewrite text, so they must be rewritten before the
/// passes that measure it. A reference such as `docs.1` grows into
/// `docs.[^1]`, and [`walk_fences_and_tables`] lays a cell out from the text it
/// can see, so converting afterwards leaves the cell wider than the delimiter
/// row that was measured from it. A body cell reading `see docs.1` produced a
/// delimiter row of ten dashes on the first pass and of thirteen on the second,
/// and the two never agreed.
///
/// Renumbering the labels belongs here too, for the same reason: a label
/// narrows, because `[^10]` becomes `[^1]` once the distinct references are
/// numbered by first encounter. Renumbering after the wrap measured the longer
/// label left a line that the next pass rejoined, since `[^1]` fits where
/// `[^10]` did not.
///
/// The inline half is told whether the heading pass will run, and leaves the
/// text that pass will read as a Setext heading alone.
///
/// The label half's scan reaches further than its name suggests. A trailing
/// list item that a reference points at is promoted to a definition header in
/// the same scan, because the two are matched by the number they share (a
/// reference ending `.3` and the item `3.` are one footnote), and the header is
/// a longer marker than the item's own, so it has to be written before the wrap
/// measures the line as well.
///
/// Only the block half waits for the layout, and
/// [`super::process_stream_inner`] runs it there.
pub(super) fn convert_footnote_references(pre: Vec<String>, opts: Options) -> Vec<String> {
    if !opts.footnotes {
        return pre;
    }

    renumber_footnote_labels(&convert_inline_footnotes_with_setext(&pre, opts.headings))
}

/// Buffers `pre` through the fence tracker and the table buffer, reflowing
/// every table it finds.
///
/// Returns the buffered output and the lines that belong to a table, which the
/// code-emphasis pass protects from the non-table repair. Code-emphasis and
/// ellipsis both shorten table cells, so they run inside the buffer, before
/// reflow measures column widths. Non-table text remains for the passes that
/// run after the walk.
pub(super) fn walk_fences_and_tables(
    pre: Vec<String>,
    opts: Options,
) -> (Vec<String>, Vec<String>) {
    let table_substitutions = TableSubstitutions {
        ellipsis: opts.ellipsis,
        code_emphasis: opts.code_emphasis,
    };
    let mut state = ProcessBuffer::new(&table_substitutions);
    // Track fences so subsequent logic respects shared semantics.
    let mut fence_tracker = FenceTracker::default();

    for line in pre {
        let fence = fence_tracker.observe_source_line(&line);
        if state.handle_fence_line(&line, fence.is_fence_marker) {
            continue;
        }

        if fence.is_in_fence {
            state.push_out(line);
            continue;
        }

        let Some(line) = state.handle_table_line(line) else {
            continue;
        };

        state.flush();
        state.push_out(line);
    }

    let (out, table_markers) = state.finish();
    let table_lines = out
        .iter()
        .zip(table_markers)
        .filter(|(_, is_table_line)| *is_table_line)
        .map(|(line, _)| line.clone())
        .collect::<Vec<_>>();

    (out, table_lines)
}
