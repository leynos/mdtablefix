//! Unit tests for Markdown processing.

use super::*;

#[test]
fn processes_html_and_tables() {
    let input = vec![
        "<table><tr><td>A</td><td>B</td></tr></table>".to_string(),
        "| X | Y |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    let output = process_stream(&input);
    assert!(output.iter().any(|l| l.contains("| A   | B   |")));
    assert!(output.iter().any(|l| l.contains("| X   | Y   |")));
}

#[test]
fn no_wrap_option() {
    let input = vec!["| a | b |".to_string(), "| 1 | 2 |".to_string()];
    let out = process_stream_no_wrap(&input);
    assert_eq!(out, vec!["| a | b |", "| 1 | 2 |"]);
}

#[test]
fn integrates_code_emphasis_flag() {
    let input = vec!["`X`** Y (in **`Z`**)**".to_string()];
    let out = process_stream_inner(
        &input,
        Options {
            code_emphasis: true,
            ..Default::default()
        },
    );
    assert_eq!(out, vec!["**`X` Y (in `Z`)**"]);
}

#[test]
fn converts_headings_when_enabled() {
    let input = vec![
        "Heading".to_string(),
        "====".to_string(),
        "Paragraph".to_string(),
    ];
    let disabled = process_stream_inner(
        &input,
        Options {
            headings: false,
            ..Default::default()
        },
    );
    assert_eq!(disabled, input);

    let enabled = process_stream_inner(
        &input,
        Options {
            headings: true,
            ..Default::default()
        },
    );
    assert_eq!(
        enabled,
        vec!["# Heading".to_string(), "Paragraph".to_string()]
    );
}

#[test]
fn converts_footnote_references_before_the_table_is_measured() {
    // `docs.1` grows into `docs.[^1]`, so the reference has to be rewritten
    // before the table pass measures the cell it sits in. Converting
    // afterwards left the delimiter row measured from the shorter text: ten
    // dashes on the first pass and thirteen on the second, and the two never
    // agreed.
    let input = vec![
        "| a | see docs.1 |".to_string(),
        "| --- | --- |".to_string(),
    ];
    let out = process_stream_inner(
        &input,
        Options {
            footnotes: true,
            ..Default::default()
        },
    );

    assert_eq!(
        out,
        vec![
            "| a   | see docs.[^1] |".to_string(),
            "| --- | ------------- |".to_string(),
        ]
    );
}

#[test]
fn renumbers_footnote_labels_before_the_wrap_measures_them() {
    // A reference keeps the number it is given by first encounter, so a long
    // label narrows as the document is renumbered: `[^10]` becomes `[^1]`. The
    // wrap measures the line the reference sits on, so the label has to be
    // rewritten before it. Seventy-five `w` followed by `[^10]` is
    // eighty-one columns and wrapped; the same line ending `[^1]` is
    // seventy-nine and is not, so renumbering afterwards left a first pass the
    // second rejoined.
    let long = format!("{} [^10]", "w".repeat(75));
    let short = format!("{} [^1]", "w".repeat(75));
    let options = Options {
        wrap: true,
        footnotes: true,
        ..Default::default()
    };

    let once = process_stream_inner(&[long], options);

    assert_eq!(once, vec![short]);
    assert_eq!(process_stream_inner(&once, options), once);
}

#[test]
fn process_stream_inner_applies_table_ellipsis_before_reflow() {
    let input = vec![
        "| example | value |".to_string(),
        "| ------- | ----- |".to_string(),
        "| ... | tail |".to_string(),
    ];

    let with_ellipsis = process_stream_inner(
        &input,
        Options {
            ellipsis: true,
            ..Default::default()
        },
    );
    let without_ellipsis = process_stream_inner(&input, Options::default());

    assert!(with_ellipsis.iter().any(|line| line.contains('…')));
    assert!(!with_ellipsis.iter().any(|line| line.contains("...")));
    assert!(without_ellipsis.iter().any(|line| line.contains("...")));
    assert!(!without_ellipsis.iter().any(|line| line.contains('…')));
}
