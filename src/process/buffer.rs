//! Line buffering and table-flush state machine for stream processing.
//!
//! [`ProcessBuffer`] accumulates lines while [`process_stream_inner`] walks
//! the input, deciding when a run of lines forms a Markdown table and should
//! be reflowed. It is kept in its own module so the orchestration in the
//! parent [`process`](super) module stays within the repository size limit.

use crate::{
    ellipsis::replace_ellipsis,
    table::reflow_table,
    wrap::{FenceTracker, LinkReferenceMatcher, classify_block},
};

/// Flushes buffered lines to `out`, formatting as a table when required.
///
/// Fields are visible to the parent module so the stream loop can construct
/// the buffer, push verbatim output, and drain the accumulated lines.
pub(super) struct ProcessBuffer {
    pub(super) out: Vec<String>,
    pub(super) buf: Vec<String>,
    pub(super) in_table: bool,
    pub(super) ellipsis: bool,
}

impl ProcessBuffer {
    pub(super) fn flush(&mut self) {
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
        if line.trim_start().starts_with('|') {
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
