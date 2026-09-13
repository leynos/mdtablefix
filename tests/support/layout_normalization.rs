//! Regression tests for normalization that must precede layout.

use super::idempotence_harness::{flags_for, format_twice};

const WRAP: u16 = 1;
const RENUMBER: u16 = 1 << 1;
const ELLIPSIS: u16 = 1 << 3;
const FOOTNOTES: u16 = 1 << 5;
const CODE_EMPHASIS: u16 = 1 << 6;

#[test]
fn footnote_conversion_precedes_wrapping_at_the_width_boundary() {
    let document = include_str!("../data/idempotence/issue_484_footnotes_wrap.dat");
    let source_line = document.lines().next().expect("fixture has prose");
    assert_eq!(source_line.chars().count(), 78);
    assert!(source_line.ends_with(".2"));

    let flags = flags_for(WRAP | FOOTNOTES);
    let (once, twice) = format_twice(document, &flags);

    assert_eq!(twice, once);
    assert!(
        once.contains("[^"),
        "footnote conversion did not run: {once:?}"
    );
    assert!(
        once.lines().all(|line| line.len() <= 80),
        "wrapping measured pre-conversion content: {once:?}"
    );
}

#[test]
fn footnote_conversion_precedes_list_renumbering() {
    let document = "See.[^7]\n\n7. Seventh\n";
    let flags = flags_for(RENUMBER | FOOTNOTES);
    let (once, twice) = format_twice(document, &flags);

    assert_eq!(twice, once);
    assert_eq!(once, "See.[^1]\n\n[^1]: Seventh\n");
}

#[test]
fn ellipsis_before_a_footnote_keeps_its_complete_dot_run() {
    let flags = flags_for(ELLIPSIS | FOOTNOTES);
    let (once, twice) = format_twice("Wait...2\n", &flags);

    assert_eq!(twice, once);
    assert_eq!(once, "Wait…[^1]\n");
}

#[test]
fn table_reflow_measures_footnote_converted_cells() {
    let document = "| Note |\n| ---- |\n| See.2 |\n\n## Footnotes\n\n2. Table note\n";
    let flags = flags_for(FOOTNOTES);
    let (once, twice) = format_twice(document, &flags);

    assert_eq!(twice, once);
    assert_eq!(
        once,
        concat!(
            "| Note     |\n",
            "| -------- |\n",
            "| See.[^1] |\n",
            "\n",
            "## Footnotes\n",
            "\n",
            "[^1]: Table note\n",
        )
    );
}

#[test]
fn list_renumbering_precedes_wrapping_at_the_digit_width_boundary() {
    let document = include_str!("../data/idempotence/issue_484_renumber_wrap.dat");
    let flags = flags_for(WRAP | RENUMBER);
    let (once, twice) = format_twice(document, &flags);

    assert_eq!(twice, once);
    let lines: Vec<&str> = once.lines().collect();
    let tenth = lines
        .iter()
        .position(|line| line.starts_with("10. "))
        .expect("renumbered output has a tenth item");
    let continuations = &lines[tenth + 1..];
    assert!(!continuations.is_empty(), "the tenth item must wrap");
    assert!(continuations.iter().all(|line| {
        line.starts_with("    ")
            && line
                .chars()
                .take_while(|character| *character == ' ')
                .count()
                == 4
    }));
}

#[test]
fn table_reflow_measures_code_emphasis_repaired_cells() {
    let document = include_str!("../data/cli-matrix/table-prose.dat");
    let flags = flags_for(CODE_EMPHASIS);
    let (once, twice) = format_twice(document, &flags);

    assert_eq!(twice, once);
    let table: Vec<&str> = once.lines().take(3).collect();
    assert_eq!(
        table,
        [
            "| Name  | Notes                              |",
            "| ----- | ---------------------------------- |",
            "| alpha | Use `cargo test` before merging... |",
        ]
    );
}
