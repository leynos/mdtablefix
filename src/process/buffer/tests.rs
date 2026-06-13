//! Unit tests for the [`ProcessBuffer`](super::ProcessBuffer) table-flush
//! state machine.

use rstest::rstest;

use super::*;

/// Builds a fresh, empty buffer with table reflow enabled and ellipsis
/// replacement disabled (the default for these tests).
fn new_buffer() -> ProcessBuffer {
    ProcessBuffer {
        out: Vec::new(),
        buf: Vec::new(),
        in_table: false,
        ellipsis: false,
    }
}

fn owned(lines: &[&str]) -> Vec<String> { lines.iter().map(|l| (*l).to_string()).collect() }

#[test]
fn plain_table_line_enters_table_mode() {
    let mut buffer = new_buffer();

    let accepted = buffer.handle_table_line("| a | b |");

    assert!(accepted);
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

    let accepted = buffer.handle_table_line(line);

    assert!(!accepted);
    assert!(!buffer.in_table);
    assert!(buffer.buf.is_empty());
}

#[test]
fn empty_line_flushes_active_table() {
    let mut buffer = new_buffer();
    buffer.handle_table_line("| a | b |");

    let accepted = buffer.handle_table_line("");

    assert!(!accepted);
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
    buffer.handle_table_line("| a | b |");

    let accepted = buffer.handle_table_line(block_line);

    assert!(
        !accepted,
        "block line should not be accepted as a table row"
    );
    assert!(!buffer.in_table, "block boundary should leave table mode");
    assert!(buffer.buf.is_empty(), "buffer should be flushed");
    // The flushed table reaches `out`; the block line itself is left for the
    // caller to handle (it is not emitted by `handle_table_line`).
    assert_eq!(buffer.out, owned(&["| a | b |"]));
}

#[test]
fn plain_pipe_continuation_is_buffered() {
    let mut buffer = new_buffer();
    buffer.handle_table_line("| a | b |");

    // No leading pipe and not block-prefixed, but contains `|`: a genuine
    // continuation row that belongs in the table buffer.
    let accepted = buffer.handle_table_line("c | d");

    assert!(accepted);
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
fn table_continuation_then_block_line_splits_correctly() {
    let mut buffer = new_buffer();

    assert!(buffer.handle_table_line("| a | b |"));
    assert!(buffer.handle_table_line("| --- | --- |"));
    assert!(buffer.handle_table_line("| 1 | 2 |"));
    // A block-prefixed line bearing a pipe ends the table run.
    assert!(!buffer.handle_table_line("- note | x"));

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

    assert!(buffer.handle_table_line("| a | b |"));
    assert!(buffer.handle_table_line("| --- | --- |"));
    assert!(buffer.handle_table_line("| 1 | 2 |"));
    assert!(!buffer.handle_table_line("    | indented | code |"));

    assert!(!buffer.in_table);
    assert!(buffer.buf.is_empty());
    assert_eq!(
        buffer.out,
        owned(&["| a   | b   |", "| --- | --- |", "| 1   | 2   |"]),
    );
}
