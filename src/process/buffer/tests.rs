//! Unit tests for the [`ProcessBuffer`](super::ProcessBuffer) table-flush
//! state machine.

use rstest::{fixture, rstest};

use super::*;
use crate::code_emphasis::fix_code_emphasis;

/// Builds a fresh, empty buffer with table substitutions disabled.
fn new_buffer() -> ProcessBuffer {
    ProcessBuffer {
        out: Vec::new(),
        table_lines: Vec::new(),
        buf: Vec::new(),
        in_table: false,
        ellipsis: false,
        code_emphasis: false,
    }
}

/// Builds a fresh buffer with code-emphasis substitution enabled.
#[fixture]
fn new_buffer_with_code_emphasis() -> ProcessBuffer {
    ProcessBuffer {
        out: Vec::new(),
        table_lines: Vec::new(),
        buf: Vec::new(),
        in_table: false,
        ellipsis: false,
        code_emphasis: true,
    }
}

fn owned(lines: &[&str]) -> Vec<String> { lines.iter().map(|l| (*l).to_string()).collect() }

fn handle_line(buffer: &mut ProcessBuffer, line: &str) -> Option<String> {
    buffer.handle_table_line(line.to_string())
}

#[test]
fn plain_table_line_enters_table_mode() {
    let mut buffer = new_buffer();

    let passthrough = handle_line(&mut buffer, "| a | b |");

    assert!(passthrough.is_none());
    assert!(buffer.in_table);
    assert_eq!(buffer.buf, owned(&["| a | b |"]));
    assert!(buffer.out.is_empty());
}

#[rstest]
#[case::four_spaces("    | not | a | table |")]
#[case::leading_tab("\t| not | a | table |")]
fn indented_code_block_line_does_not_enter_table_mode(#[case] line: &str) {
    // Four or more columns of indentation marks an indented code block; it must
    // stay verbatim rather than entering table mode and being reflowed.
    let mut buffer = new_buffer();

    let passthrough = handle_line(&mut buffer, line);

    assert_eq!(passthrough, Some(line.to_string()));
    assert!(!buffer.in_table);
    assert!(buffer.buf.is_empty());
}

#[test]
fn empty_line_flushes_active_table() {
    let mut buffer = new_buffer();
    handle_line(&mut buffer, "| a | b |");

    let passthrough = handle_line(&mut buffer, "");

    assert_eq!(passthrough, Some(String::new()));
    assert!(!buffer.in_table);
    assert!(buffer.buf.is_empty());
    assert_eq!(buffer.out, owned(&["| a | b |"]));
}

#[rstest]
#[case::bullet("- item | value")]
#[case::link_reference("[ref]: url|alt")]
#[case::blockquote("> quote | with pipe")]
#[case::footnote("[^id]: note | with pipe")]
fn block_prefixed_pipe_line_flushes_table(#[case] block_line: &str) {
    // Regression for the logic-order bug: a block marker that carries its own
    // `|` must be recognised as a new block and flush the active table run,
    // not be absorbed into it by the `line.contains('|')` continuation check.
    let mut buffer = new_buffer();
    handle_line(&mut buffer, "| a | b |");

    let passthrough = handle_line(&mut buffer, block_line);

    assert_eq!(passthrough, Some(block_line.to_string()));
    assert!(!buffer.in_table, "block boundary should leave table mode");
    assert!(buffer.buf.is_empty(), "buffer should be flushed");
    // The flushed table reaches `out`; the block line itself is left for the
    // caller to handle (it is not emitted by `handle_table_line`).
    assert_eq!(buffer.out, owned(&["| a | b |"]));
}

#[test]
fn plain_pipe_continuation_is_buffered() {
    let mut buffer = new_buffer();
    handle_line(&mut buffer, "| a | b |");

    // No leading pipe and not block-prefixed, but contains `|`: a genuine
    // continuation row that belongs in the table buffer.
    let passthrough = handle_line(&mut buffer, "c | d");

    assert!(passthrough.is_none());
    assert!(buffer.in_table);
    assert_eq!(buffer.buf, owned(&["| a | b |", "c | d"]));
    assert!(buffer.out.is_empty());
}

#[test]
fn flush_empty_buffer_is_noop() {
    let mut buffer = new_buffer();
    buffer.in_table = true;

    buffer.flush();

    assert!(buffer.out.is_empty());
    assert!(buffer.buf.is_empty());
    // The empty-buffer guard returns before resetting `in_table`, so the call
    // genuinely changes nothing.
    assert!(buffer.in_table);
}

#[test]
fn flush_non_table_emits_lines_verbatim() {
    let mut buffer = new_buffer();
    buffer.buf = owned(&["plain text", "more text"]);
    buffer.in_table = false;

    buffer.flush();

    assert_eq!(buffer.out, owned(&["plain text", "more text"]));
    assert!(buffer.buf.is_empty());
}

#[test]
fn flush_table_passes_lines_through_reflow() {
    let input = owned(&["| a | b |", "| --- | --- |", "| 1 | 2 |"]);
    let mut buffer = new_buffer();
    buffer.buf = input.clone();
    buffer.in_table = true;

    buffer.flush();

    let expected = owned(&["| a   | b   |", "| --- | --- |", "| 1   | 2   |"]);
    assert_eq!(buffer.out, expected);
    assert_eq!(buffer.out, reflow_table(&input));
    assert_ne!(buffer.out, input, "reflow should normalise column widths");
    assert!(!buffer.in_table);
}

#[test]
fn finish_flushes_a_table_that_ends_at_end_of_input() {
    let mut buffer = new_buffer();

    assert!(handle_line(&mut buffer, "| a | b |").is_none());
    assert!(handle_line(&mut buffer, "| --- | --- |").is_none());
    assert!(handle_line(&mut buffer, "| 1 | 2 |").is_none());

    let result = buffer.finish();

    assert_eq!(
        result,
        owned(&["| a   | b   |", "| --- | --- |", "| 1   | 2   |"]),
    );
}

#[rstest]
fn flush_table_applies_code_emphasis_before_reflow(
    mut new_buffer_with_code_emphasis: ProcessBuffer,
) {
    let input = owned(&[
        "| Name  | Notes                      |",
        "| ----- | -------------------------- |",
        "| alpha | Use *`cargo test`* to run. |",
    ]);
    new_buffer_with_code_emphasis.buf = input.clone();
    new_buffer_with_code_emphasis.in_table = true;

    new_buffer_with_code_emphasis.flush();

    assert_eq!(
        new_buffer_with_code_emphasis.out,
        reflow_table(&fix_code_emphasis(&input))
    );
}

#[test]
fn table_continuation_then_block_line_splits_correctly() {
    let mut buffer = new_buffer();

    assert!(handle_line(&mut buffer, "| a | b |").is_none());
    assert!(handle_line(&mut buffer, "| --- | --- |").is_none());
    assert!(handle_line(&mut buffer, "| 1 | 2 |").is_none());
    // A block-prefixed line bearing a pipe ends the table run.
    assert_eq!(
        handle_line(&mut buffer, "- note | x"),
        Some("- note | x".to_string())
    );

    assert!(!buffer.in_table);
    assert!(buffer.buf.is_empty());
    assert_eq!(
        buffer.out,
        owned(&["| a   | b   |", "| --- | --- |", "| 1   | 2   |"]),
    );
}

#[test]
fn table_followed_by_indented_pipe_line_flushes_table() {
    let mut buffer = new_buffer();

    assert!(handle_line(&mut buffer, "| a | b |").is_none());
    assert!(handle_line(&mut buffer, "| --- | --- |").is_none());
    assert!(handle_line(&mut buffer, "| 1 | 2 |").is_none());
    assert_eq!(
        handle_line(&mut buffer, "    | indented | code |"),
        Some("    | indented | code |".to_string())
    );

    assert!(!buffer.in_table);
    assert!(buffer.buf.is_empty());
    assert_eq!(
        buffer.out,
        owned(&["| a   | b   |", "| --- | --- |", "| 1   | 2   |"]),
    );
}
