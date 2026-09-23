//! Direct coverage for the definition-scanning helpers in
//! [`super`](super).

use std::collections::HashMap;

use rstest::rstest;

use super::{
    DefinitionLine,
    assign_new_number,
    collect_definition_updates,
    definition_segment_end,
    rewrite_definition_headers,
    should_convert_numeric_line,
};

fn strings(lines: &[&str]) -> Vec<String> { lines.iter().map(|line| (*line).to_owned()).collect() }

#[test]
fn assign_new_number_reuses_existing_mapping() {
    let mut mapping = HashMap::from([(7, 2)]);
    let mut next_number = 3;

    assert_eq!(assign_new_number(&mut mapping, 7, &mut next_number), 2);
    assert_eq!(assign_new_number(&mut mapping, 9, &mut next_number), 3);
    assert_eq!(mapping.get(&9), Some(&3));
    assert_eq!(next_number, 4);
}

#[rstest]
#[case(2, Some((1, 4)), false, true)]
#[case(4, Some((1, 4)), false, false)]
#[case(2, Some((1, 4)), true, false)]
#[case(2, None, false, false)]
fn should_convert_numeric_line_respects_range_and_skip_flag(
    #[case] index: usize,
    #[case] range: Option<(usize, usize)>,
    #[case] skip: bool,
    #[case] expected: bool,
) {
    assert_eq!(should_convert_numeric_line(index, range, skip), expected);
}

#[test]
fn definition_segment_end_includes_continuations_and_separating_blanks() {
    let lines = strings(&[
        "[^1]: First",
        "    continuation",
        "",
        "still part",
        "[^2]: Second",
    ]);

    assert_eq!(definition_segment_end(&lines, 0, lines.len()), 3);
}

#[test]
fn collect_definition_updates_rewrites_existing_definitions() {
    let lines = strings(&["Reference.[^7]", "", "[^7]: Existing"]);
    let mut mapping = HashMap::from([(7, 1)]);

    let updates = collect_definition_updates(&lines, &mut mapping);

    assert_eq!(updates.is_definition_line, vec![false, false, true]);
    assert_eq!(
        updates
            .definitions
            .iter()
            .map(|definition| definition.line.as_str())
            .collect::<Vec<_>>(),
        vec!["[^1]: Existing"]
    );
}

/// The item a reference reaches takes its number; the items it does not reach
/// take the pool in the order they were written, so the block still reads the
/// way the list was authored once it is sorted.
#[rstest]
#[case::single_candidate(
    strings(&["Reference.[^7]", "", "9. Numeric note"]),
    vec![false, false, true],
    vec!["[^2]: Numeric note"]
)]
#[case::scan_order(
    strings(&[
        "Reference.[^7]",
        "",
        "1. First note",
        "2. Second note",
        "7. Seventh note",
    ]),
    vec![false, false, true, true, true],
    vec!["[^2]: First note", "[^3]: Second note", "[^1]: Seventh note"]
)]
fn collect_definition_updates_numbers_numeric_candidates(
    #[case] lines: Vec<String>,
    #[case] expected_flags: Vec<bool>,
    #[case] expected_lines: Vec<&str>,
) {
    let mut mapping = HashMap::from([(7, 1)]);

    let updates = collect_definition_updates(&lines, &mut mapping);

    assert_eq!(updates.is_definition_line, expected_flags);
    assert_eq!(
        updates
            .definitions
            .iter()
            .map(|definition| definition.line.as_str())
            .collect::<Vec<_>>(),
        expected_lines
    );
}

#[test]
fn rewrite_definition_headers_updates_only_known_definition_lines() {
    let mut lines = strings(&["[^7]: Old", "text"]);
    let definitions = vec![DefinitionLine {
        index: 0,
        new_number: 1,
        line: "[^1]: New".to_owned(),
    }];

    rewrite_definition_headers(&mut lines, &definitions);

    assert_eq!(lines, strings(&["[^1]: New", "text"]));
}
