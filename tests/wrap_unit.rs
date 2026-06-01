//! Unit tests for `wrap_text`.
//!
//! The original `wrap_unit.rs` file outgrew the 400-line repository limit, so
//! the cases are split by concern into focused submodules under
//! `tests/wrap_unit/`. Each submodule pulls in `lines_vec!` and any other
//! helpers it needs.

#[macro_use]
#[path = "common/mod.rs"]
mod common;

fn wrap_text_oversized_code_span_stays_intact() {
    let code_span = concat!(
        "`fn find_interrupted_session(base_dir: &Utf8Path, owner: &str, ",
        "repository: &str, pr_number: u64) -> Result<Option<SessionState>, IntakeError>`",
    );
    let input = lines_vec![format!("1. Implement {code_span} now.")];
    let wrapped = wrap_text(&input, 80);

    assert!(wrapped.join("\n").contains(code_span));
    assert!(wrapped.iter().all(|line| !line.trim().is_empty()));
    assert!(wrapped.iter().skip(1).all(|line| line.starts_with("   ")));
}

#[path = "wrap_unit/code_spans.rs"]
mod code_spans;
#[path = "wrap_unit/footnotes.rs"]
mod footnotes;
#[path = "wrap_unit/prefixed.rs"]
mod prefixed;
#[path = "wrap_unit/shell_blocks.rs"]
mod shell_blocks;
#[path = "wrap_unit/stream.rs"]
mod stream;
