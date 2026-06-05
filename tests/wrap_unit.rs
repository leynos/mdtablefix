//! Unit tests for `wrap_text`.
//!
//! The original `wrap_unit.rs` file outgrew the 400-line repository limit, so
//! the cases are split by concern into focused submodules under
//! `tests/wrap_unit/`. Each submodule pulls in `lines_vec!` and any other
//! helpers it needs.

#[macro_use]
#[path = "common/mod.rs"]
mod common;

#[path = "wrap_unit/code_spans.rs"]
mod code_spans;
#[path = "wrap_unit/dates.rs"]
mod dates;
#[path = "wrap_unit/footnotes.rs"]
mod footnotes;
#[path = "wrap_unit/prefixed.rs"]
mod prefixed;
#[path = "wrap_unit/shell_blocks.rs"]
mod shell_blocks;
#[path = "wrap_unit/stream.rs"]
mod stream;
