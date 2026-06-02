//! Regression and integration tests for inline code spans that are
//! soft-wrapped across source lines.
//!
//! These tests cover the `PendingPrefix` deferral mechanism introduced in
//! `src/wrap/paragraph.rs` and the orchestration logic in `src/wrap.rs`
//! (`handle_pending_continuation`, `update_span_state`).
//!
//! Fixture-driven cases (`test_wrap_spanning_code_span_fixtures`) use input
//! files from `tests/data/` and are verified against committed `insta`
//! snapshots in `tests/snapshots/`. Focused unit tests assert structural
//! invariants (no orphaned closing backticks, prefix preservation, hard-break
//! retention) that complement the snapshot coverage.
//!
//! Related modules:
//! - `tests/wrap_unit.rs` — table-driven unit tests for `wrap_text`
//! - `tests/wrap_properties.rs` — property-based tests for the same invariants
//! - `src/wrap/tests/span_state.rs` — unit tests for scanning helpers

use mdtablefix::wrap::wrap_text;
use rstest::rstest;
use unicode_width::UnicodeWidthStr;

use super::*;

#[rstest]
#[case(
    "1_",
    include_lines!("../data/spanning_code_span_ordered_input.txt"),
)]
#[case(
    "-",
    include_lines!("../data/spanning_code_span_bullet_input.txt"),
)]
fn test_wrap_spanning_code_span_fixtures(#[case] prefix: &str, #[case] input: Vec<String>) {
    let output = process_stream(&input);
    insta::with_settings!({
        snapshot_path => "../snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!(
            format!("spanning_code_span_{}", prefix.trim()),
            output.join("\n")
        );
    });
}

#[test]
fn test_wrap_spanning_code_span_nested_blockquote_prefixes_do_not_merge() {
    let input = lines_vec!["> Outer (`open", ">> Inner continues` text.",];
    let output = wrap_text(&input, 80);
    assert!(output.len() >= 2);
    assert!(output[0].starts_with("> Outer"));
    assert!(
        output.iter().any(|line| line.starts_with(">>")),
        "nested blockquote prefix must be preserved: {output:?}"
    );
}

#[test]
fn test_wrap_spanning_code_span_three_line_blockquote() {
    let input = lines_vec![
        "> Theme selection uses layered configuration (`CLI >",
        "> environment > config file >",
        "> defaults`) with OrthoConfig-backed parsing.",
    ];
    let output = process_stream(&input);
    assert!(output.len() >= 2);
    assert!(output.iter().all(|line| line.starts_with('>')));
    let rendered = output.join("\n");
    assert!(rendered.contains("(`CLI > environment > config file > defaults`)"));
}

#[test]
fn test_wrap_spanning_code_span_footnote_definition() {
    let input = lines_vec![
        "[^note]: The default interpreter is `powershell -Command` on Windows and",
        "  `bash` elsewhere unless overridden by the manifest `interpreter` field.",
    ];
    let output = process_stream(&input);
    assert!(output.len() >= 2);
    assert!(output[0].starts_with("[^note]: "));
    let rendered = output.join("\n");
    assert!(rendered.contains("`powershell -Command`"));
    assert!(rendered.contains("`bash`"));
    assert!(rendered.contains("`interpreter`"));
}

#[test]
fn test_wrap_joins_unclosed_span_continuation() {
    let input = lines_vec!["- `open", " span` closes."];
    let output = wrap_text(&input, 80);
    assert_eq!(output.len(), 1);
    assert!(output[0].contains("`open span`"));
}

#[test]
fn test_wrap_preserves_hard_break_when_buffered_span_closes() {
    let input = lines_vec!["- `foo", "bar` continues.  ", "next"];
    let output = wrap_text(&input, 80);
    assert_eq!(output.len(), 2);
    assert_eq!(output[0], "- `foo bar` continues.  ");
    assert_eq!(output[1], "next");
}

#[test]
fn test_wrap_defers_while_any_span_stays_open() {
    let input = lines_vec!["- `done` and `open", " span` continues."];
    let output = wrap_text(&input, 80);
    let rendered = output.join("\n");
    assert!(rendered.contains("`done`"));
    assert!(rendered.contains("`open span`"));
}

#[test]
fn test_wrap_pending_cleared_after_span_closes_on_continuation() {
    // Span closes on the continuation line; the pending buffer stays alive
    // only until the scanner confirms no span remains open. The following
    // plain line starts a new paragraph instead of extending the pending
    // prefix buffer.
    let input = lines_vec!["- `foo", "  bar`", "baz"];
    let output = wrap_text(&input, 80);
    assert_eq!(output, vec!["- `foo bar`".to_string(), "baz".to_string()],);
}

#[test]
fn test_wrap_reopen_with_whitespace_tail_is_split_atomically() {
    // Without a real closing fence for span B, preserving the source is safer
    // than inventing Markdown that changes the prose.
    let input = lines_vec!["- foo `open", "  closed` and ` more"];
    let output = wrap_text(&input, 80);
    let rendered = output.join("\n");
    assert_eq!(output, input);
    assert!(!rendered.contains("` more`"));
}

#[test]
fn test_wrap_reopen_with_close_paren_tail_is_split_atomically() {
    // Same as the whitespace case, but the new opener is immediately
    // followed by ')'.
    let input = lines_vec!["- foo `open", "  closed` and `)done"];
    let output = wrap_text(&input, 80);
    let rendered = output.join("\n");
    assert_eq!(output, input);
    assert!(!rendered.contains("`)done`"));
}

#[test]
fn test_wrap_prefixed_open_span_leaves_indented_code_verbatim() {
    let input = lines_vec!["- `open", "    --version", "text"];
    let output = process_stream(&input);
    let rendered = output.join("\n");

    assert!(output.iter().any(|line| line == "    --version"));
    assert!(
        !rendered.contains("`open --version"),
        "indented line was merged into span text: {rendered:?}"
    );
}

#[test]
fn test_wrap_three_line_close_a_open_b_close_b() {
    let input = lines_vec![
        "4. `parse_rsa_pem(pem_contents, display_path) ->",
        "   Result<EncodingKey, GitHubError>` calls `validate_rsa_pem",
        "   first`, then maps the error.",
    ];
    let output = wrap_text(&input, 80);
    let rendered = output.join("\n");

    assert!(
        rendered.contains(
            "`parse_rsa_pem(pem_contents, display_path) -> Result<EncodingKey, GitHubError>`"
        ),
        "span A must remain intact: {rendered:?}"
    );
    assert!(
        rendered.contains("`validate_rsa_pem first`"),
        "span B must remain intact: {rendered:?}"
    );
    assert_no_md038_code_span(&rendered);
}

#[test]
fn test_wrap_opener_at_eol_emits_verbatim() {
    let input = lines_vec![
        "4. Calls the configured loader `",
        "   EncodingKey::from_rsa_pem(pem_contents.as_bytes())`, mapping errors.",
    ];
    let output = wrap_text(&input, 80);
    let rendered = output.join("\n");

    assert!(rendered.contains("`EncodingKey::from_rsa_pem(pem_contents.as_bytes())`"));
    assert_no_md038_code_span(&rendered);
}

#[test]
fn test_wrap_preserves_authored_edges_of_multi_backtick_code_span() {
    let input = lines_vec!["- `` foo `", "  bar ` baz ``"];
    let output = wrap_text(&input, 80);
    let rendered = output.join("\n");

    assert!(
        rendered.contains("`` foo ` bar ` baz ``"),
        "authored multi-backtick content must be preserved: {rendered:?}"
    );
    assert!(
        !rendered.contains("`bar`"),
        "single-backtick runs inside a two-backtick span are content: {rendered:?}"
    );
}

#[test]
fn test_wrap_issue_md038_regression_fixture() {
    let input: Vec<String> = include_lines!("../data/issue_md038_input.txt");
    let expected: Vec<String> = include_lines!("../data/issue_md038_expected.txt");
    let output = process_stream(&input);

    assert_eq!(output, expected);
    assert_no_md038_code_span(&output.join("\n"));
}

#[test]
fn test_wrap_does_not_join_overlong_signature() {
    let input = lines_vec![
        "- `EngineConnector::connect(socket: impl AsRef<str>)",
        "  -> Result<Docker, PodbotError>`",
    ];
    let output = wrap_text(&input, 80);

    assert_eq!(output, input);
    assert_no_line_exceeds_width(&output, 80);
}

#[test]
fn test_wrap_verbatim_width_guard_keeps_hard_break_on_continuation() {
    let input = lines_vec![
        "- `EngineConnector::connect(socket: impl AsRef<str>)",
        "  -> Result<Docker, PodbotError>`  ",
        "next",
    ];
    let output = wrap_text(&input, 80);

    assert_eq!(output, input);
    assert!(!output[0].ends_with("  "));
    assert!(output[1].ends_with("  "));
    assert_no_line_exceeds_width(&output, 80);
}

#[test]
fn test_wrap_verbatim_width_guard_preserves_raw_continuation_line() {
    let input = lines_vec![
        "- `EngineConnector::connect(socket: impl AsRef<str>)",
        "      -> Result<Docker, PodbotError>`",
    ];
    let output = wrap_text(&input, 80);

    assert_eq!(output, input);
}

#[test]
fn test_wrap_verbatim_flush_width_guard_preserves_original_lines() {
    let input = lines_vec![
        "- `first span starts here",
        concat!(
            "  and closes here` then opens ",
            "`SecondSpan::with_a_long_signature(first: FirstType, second: SecondType,"
        ),
        "  third: ThirdType, fourth: FourthType)`",
    ];
    let output = wrap_text(&input, 80);

    assert_eq!(output, input);
}

#[test]
fn test_assert_no_md038_code_span_handles_multi_backtick_content() {
    assert_no_md038_code_span("``foo ` bar ` baz``");
}

#[test]
fn test_wrap_does_not_join_overlong_import_list() {
    let input = lines_vec![
        "- `podbot::engine::{ContainerSecurityOptions, CreateContainerRequest,",
        "  EngineConnector, SelinuxLabelMode}`",
    ];
    let output = wrap_text(&input, 80);

    assert_eq!(output, input);
    assert_no_line_exceeds_width(&output, 80);
}

#[test]
fn test_wrap_partial_join_then_verbatim() {
    let input = lines_vec![
        "- `Api::call(",
        "  first: FirstType, second: SecondType,",
        "  third: ThirdType, fourth: FourthType)`",
    ];
    let output = wrap_text(&input, 80);

    assert_eq!(
        output,
        vec![
            "- `Api::call( first: FirstType, second: SecondType,".to_string(),
            "  third: ThirdType, fourth: FourthType)`".to_string(),
        ]
    );
    assert_no_line_exceeds_width(&output, 80);
}

fn assert_no_md038_code_span(rendered: &str) {
    let mut remaining = rendered;
    while let Some(open_index) = remaining.find('`') {
        let fence_len = backtick_run_len(&remaining[open_index..]);
        let after_open = &remaining[open_index + fence_len..];
        let Some(close_index) = matching_backtick_run_index(after_open, fence_len) else {
            break;
        };
        let code = &after_open[..close_index];
        assert!(
            code.is_empty() || (!code.starts_with(' ') && !code.ends_with(' ')),
            "code span has leading or trailing space: {code:?} in {rendered:?}"
        );
        remaining = &after_open[close_index + fence_len..];
    }
}

fn matching_backtick_run_index(text: &str, fence_len: usize) -> Option<usize> {
    let mut search = 0;
    while let Some(relative_index) = text[search..].find('`') {
        let run_start = search + relative_index;
        let run_len = backtick_run_len(&text[run_start..]);
        if run_len == fence_len {
            return Some(run_start);
        }
        search = run_start + run_len;
    }
    None
}

fn backtick_run_len(text: &str) -> usize {
    text.as_bytes()
        .iter()
        .take_while(|byte| **byte == b'`')
        .count()
}

fn assert_no_line_exceeds_width(output: &[String], width: usize) {
    for line in output {
        let line_width = UnicodeWidthStr::width(line.as_str());
        assert!(
            line_width <= width,
            "line exceeds {width} columns with width {line_width}: {line:?}"
        );
    }
}
