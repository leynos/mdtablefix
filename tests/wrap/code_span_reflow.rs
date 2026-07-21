//! Regression tests for reflowing code spans split across source lines.

use mdtablefix::{process::WRAP_COLS, wrap::wrap_text};
use rstest::rstest;
use unicode_width::UnicodeWidthStr;

#[rstest]
#[case::plain_list(
    lines_vec![
        "- The framing shipped as a `code span that is wrapped across the",
        "  line boundary` on both guides, satisfying the stated callout",
        "  requirement that was promoted from a single sentence during the review.",
    ],
    lines_vec![
        "- The framing shipped as a `code span that is wrapped across the line boundary`",
        "  on both guides, satisfying the stated callout requirement that was promoted",
        "  from a single sentence during the review.",
    ],
)]
#[case::hyphenated_tail(
    lines_vec![
        "- The v0.6/v0.7 framing shipped as a `> **Note: this is a v0.6 interim",
        "  workaround.**` admonition on both guides, satisfying the set-off-",
        "  callout requirement that was promoted from a single-sentence opener",
        "  during the Logisphere revision.",
    ],
    lines_vec![
        "- The v0.6/v0.7 framing shipped as a",
        "  `> **Note: this is a v0.6 interim workaround.**` admonition on both guides,",
        "  satisfying the set-off- callout requirement that was promoted from a",
        "  single-sentence opener during the Logisphere revision.",
    ],
)]
fn cross_line_code_span_reflow_is_idempotent(
    #[case] input: Vec<String>,
    #[case] expected: Vec<String>,
) {
    let once = wrap_text(&input, WRAP_COLS);

    assert_eq!(once, expected);
    assert_eq!(wrap_text(&once, WRAP_COLS), once);
}

#[test]
fn overlong_cross_line_code_span_preserves_conforming_source_lines() {
    let input = lines_vec![
        "This plan covers roadmap item 4.1.1 only:",
        "`Implement backend/crates/pagination providing opaque cursor encoding,",
        "PageParams, and Paginated<T> envelopes with navigation links, backed by unit",
        "tests for cursor round-tripping.`",
    ];

    let output = wrap_text(&input, WRAP_COLS);

    assert_eq!(output, input);
    assert!(
        output
            .iter()
            .all(|line| UnicodeWidthStr::width(line.as_str()) <= WRAP_COLS)
    );
}
