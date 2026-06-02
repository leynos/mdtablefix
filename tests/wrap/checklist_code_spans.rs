//! Regression tests for checklist items with cross-line inline code spans.
//!
//! Issue `#310` exposed two coupled wrapping failures: continuation joins could
//! alter inline code content, and mid-item flushes could reuse the checklist
//! marker as though a new item had started.

use mdtablefix::wrap::wrap_text;
use rstest::rstest;

use super::process_stream;

const STANDARD_WRAP_WIDTH: usize = 80;

#[rstest]
#[case(
    "multiline",
    include_lines!("../data/checklist_code_span_multiline_input.txt"),
    include_lines!("../data/checklist_code_span_multiline_expected.txt"),
)]
#[case(
    "reopened",
    include_lines!("../data/checklist_code_span_reopened_input.txt"),
    include_lines!("../data/checklist_code_span_reopened_expected.txt"),
)]
#[case(
    "function_call",
    include_lines!("../data/checklist_code_span_function_input.txt"),
    include_lines!("../data/checklist_code_span_function_expected.txt"),
)]
fn checklist_code_span_fixtures(
    #[case] name: &str,
    #[case] input: Vec<String>,
    #[case] expected: Vec<String>,
) {
    let output = process_stream(&input);
    assert_eq!(output, expected);
    insta::with_settings!({
        snapshot_path => "../snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!(
            format!("checklist_code_span_{name}"),
            output.join("\n")
        );
    });
}

#[rstest]
#[case(
    include_lines!("../data/idempotency_migration_inline_code_input.txt"),
    include_lines!("../data/idempotency_migration_inline_code_expected.txt"),
)]
#[case(
    include_lines!("../data/idempotency_checklist_code_spans_input.txt"),
    include_lines!("../data/idempotency_checklist_code_spans_expected.txt"),
)]
fn checklist_code_span_wrapping_is_idempotent(
    #[case] input: Vec<String>,
    #[case] expected: Vec<String>,
) {
    let once = wrap_text(&input, STANDARD_WRAP_WIDTH);
    assert_eq!(once, expected);
    let twice = wrap_text(&once, STANDARD_WRAP_WIDTH);
    assert_eq!(twice, once);
}
