//! Line buffering and table-flush state machine for stream processing.
//!
//! [`ProcessBuffer`] accumulates lines while [`process_stream_inner`] walks
//! the input, deciding when a run of lines forms a Markdown table and should
//! be reflowed. It is kept in its own module so the orchestration in the
//! parent [`process`](super) module stays within the repository size limit.

use tracing::debug;

use crate::{
    ellipsis::replace_ellipsis,
    table::reflow_table,
    wrap::{FenceTracker, LinkReferenceMatcher, classify_block, leading_indent},
};

// Note: `warn` is intentionally not imported. `flush` only calls
// `reflow_table` after its `buf.is_empty()` guard, and `reflow_table` returns
// an empty vector solely for empty input; for any non-empty input it yields
// either the reflowed table or the original lines verbatim. An empty result
// from a non-empty buffer is therefore unreachable, so no `warn!` is needed.

/// Flushes buffered lines to `out`, formatting as a table when required.
///
/// Fields are private; the parent module drives the buffer through the
/// narrow [`new`](Self::new), [`push_out`](Self::push_out),
/// [`handle_fence_line`](Self::handle_fence_line),
/// [`handle_table_line`](Self::handle_table_line), [`flush`](Self::flush),
/// and [`into_out`](Self::into_out) API so the table-detection invariants
/// stay encapsulated.
pub(super) struct ProcessBuffer {
    out: Vec<String>,
    buf: Vec<String>,
    in_table: bool,
    ellipsis: bool,
}

impl ProcessBuffer {
    /// Creates an empty buffer. `ellipsis` selects whether buffered table
    /// cells have `...` replaced with `…` during [`flush`](Self::flush).
    pub(super) fn new(ellipsis: bool) -> Self {
        Self {
            out: Vec::new(),
            buf: Vec::new(),
            in_table: false,
            ellipsis,
        }
    }

    /// Appends a finished line directly to the output, without touching the
    /// pending table buffer. Callers that must preserve table/verbatim
    /// ordering call [`flush`](Self::flush) first.
    pub(super) fn push_out(&mut self, line: String) { self.out.push(line); }

    /// Consumes the buffer and returns the accumulated output lines.
    ///
    /// Call [`flush`](Self::flush) beforehand to drain any pending buffered
    /// lines into the output.
    pub(super) fn into_out(self) -> Vec<String> { self.out }

    pub(super) fn flush(&mut self) {
        debug!(
            in_table = self.in_table,
            lines = self.buf.len(),
            "ProcessBuffer::flush"
        );
        if self.buf.is_empty() {
            return;
        }
        let buffered = std::mem::take(&mut self.buf);
        if self.in_table {
            let table_lines = if self.ellipsis {
                replace_ellipsis(&buffered)
            } else {
                buffered
            };
            self.out.extend(reflow_table(&table_lines));
        } else {
            self.out.extend(buffered);
        }
        self.in_table = false;
    }

    pub(super) fn push_verbatim(&mut self, line: &str) {
        self.flush();
        self.out.push(line.to_string());
    }

    pub(super) fn handle_fence_line(&mut self, line: &str, fences: &mut FenceTracker) -> bool {
        if !fences.observe(line) {
            return false;
        }

        self.push_verbatim(line);
        true
    }

    pub(super) fn handle_table_line(&mut self, line: &str) -> bool {
        // A leading indent of four or more columns marks a Markdown indented
        // code block, so such a line must stay verbatim and never enter table
        // mode (otherwise `reflow_table` would rewrite its contents). This
        // mirrors the `indent_width < 4` gate in `classify_block`.
        if leading_indent(line).0 < 4 && line.trim_start().starts_with('|') {
            debug!(line, "ProcessBuffer: table-mode on");
            self.in_table = true;
            self.buf.push(line.to_string());
            return true;
        }
        if line.trim().is_empty() {
            if self.in_table {
                self.flush();
            }
            return false;
        }
        // Recognise a new Markdown block *before* the pipe heuristic below.
        // Block markers such as `> `, `- `, or `[^id]:` may themselves contain
        // a `|`; if the pipe check ran first it would absorb such a line into
        // the table run, both corrupting the block and preventing the genuine
        // table from being reflowed (a stray non-table row makes
        // `reflow_table` bail). Flushing here keeps wrapping and table
        // detection aligned.
        if self.in_table && classify_block(line, LinkReferenceMatcher::production()).is_some() {
            debug!(line, "ProcessBuffer: flushing on block boundary");
            self.flush();
            return false;
        }
        if self.in_table && (line.contains('|') || crate::table::SEP_RE.is_match(line.trim())) {
            self.buf.push(line.to_string());
            return true;
        }
        if self.in_table {
            self.flush();
        }
        false
    }
}

#[cfg(test)]
mod tests;
