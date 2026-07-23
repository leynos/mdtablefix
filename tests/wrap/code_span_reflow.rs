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
#[case::repeated_blockquote(
    lines_vec![
        "> > The framing used a `code span that crosses the",
        "> > source boundary` on both guides, satisfying the stated callout",
        "> > requirement that was promoted during review.",
    ],
    lines_vec![
        "> > The framing used a `code span that crosses the source boundary` on both",
        "> > guides, satisfying the stated callout requirement that was promoted during",
        "> > review.",
    ],
)]
#[case::footnote_definition(
    lines_vec![
        "[^wrap]: The framing used a `code span that crosses the",
        "         source boundary` on both guides, satisfying the stated callout",
        "         requirement that was promoted during review.",
    ],
    lines_vec![
        "[^wrap]: The framing used a `code span that crosses the",
        "         source boundary` on both guides, satisfying the stated callout",
        "         requirement that was promoted during review.",
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
fn later_differently_sized_fence_preserves_overlong_span_boundaries() {
    let input = lines_vec![
        "An unmatched ` fence precedes the later code span:",
        "``Implement backend/crates/pagination providing opaque cursor encoding,",
        "PageParams, and Paginated<T> envelopes with navigation links, backed by unit",
        "tests for cursor round-tripping.``",
    ];

    let output = wrap_text(&input, WRAP_COLS);

    assert_eq!(
        output,
        lines_vec![
            "An unmatched",
            "` fence precedes the later code span:",
            "``Implement backend/crates/pagination providing opaque cursor encoding,",
            "PageParams, and Paginated<T> envelopes with navigation links, backed by unit",
            "tests for cursor round-tripping.``",
        ]
    );
    assert!(lines_conform(&output));
}

#[test]
fn overlong_spanning_code_preserves_paragraph_indent() {
    let input = lines_vec![
        "   `Implement backend/crates/pagination providing opaque cursor encoding,",
        "   PageParams, and Paginated<T> envelopes with navigation links, backed by",
        "   unit tests for cursor round-tripping.`",
    ];

    let output = wrap_text(&input, WRAP_COLS);

    assert_eq!(output, input);
    assert!(lines_conform(&output));
}

#[test]
fn mixed_hard_break_groups_preserve_eligible_span_boundaries() {
    let input = lines_vec![
        "`This first hard-break group is an intentionally overlong atomic inline-code span.`  ",
        "`Implement backend/crates/pagination providing opaque cursor encoding,",
        "PageParams, and Paginated<T> envelopes with navigation links, backed by unit",
        "tests for cursor round-tripping.`",
    ];

    let output = wrap_text(&input, WRAP_COLS);

    assert_eq!(output, input);
    assert!(UnicodeWidthStr::width(output[0].as_str()) > WRAP_COLS);
    assert!(lines_conform(&output[1..]));
}

#[test]
fn prose_outside_overlong_spanning_code_is_greedily_reflowed() {
    let input = lines_vec![
        "Introductory prose has an unnecessarily",
        "short authored boundary before the code span:",
        "`Implement backend/crates/pagination providing opaque cursor encoding,",
        "PageParams, and Paginated<T> envelopes with navigation links, backed by unit",
        "tests for cursor round-tripping.` Tail prose has another",
        "short boundary that should be reflowed.",
    ];

    let output = wrap_text(&input, WRAP_COLS);

    assert!(
        output
            .iter()
            .all(|line| line != "Introductory prose has an unnecessarily")
    );
    assert!(
        output
            .iter()
            .all(|line| line != "short boundary that should be reflowed.")
    );
    assert!(
        output
            .iter()
            .any(|line| line.starts_with("`Implement backend"))
    );
    assert!(output.iter().any(|line| line.starts_with("PageParams,")));
    assert!(lines_conform(&output));
    assert_eq!(wrap_text(&output, WRAP_COLS), output);
}

fn lines_conform(lines: &[String]) -> bool {
    lines
        .iter()
        .all(|line| UnicodeWidthStr::width(line.as_str()) <= WRAP_COLS)
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
