//! Direct coverage for the definition-scanning helpers in
//! [`super`](super).

use std::collections::HashMap;

use proptest::prelude::*;
use rstest::rstest;

use super::{
    DefinitionLine,
    assign_new_number,
    collect_definition_updates,
    definition_segment_end,
    reorder_definition_block,
    rewrite_definition_headers,
    should_convert_numeric_line,
};

fn strings(lines: &[&str]) -> Vec<String> { lines.iter().map(|line| (*line).to_string()).collect() }

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

#[test]
fn collect_definition_updates_converts_numeric_candidates() {
    let lines = strings(&["Reference.[^7]", "", "9. Numeric note"]);
    let mut mapping = HashMap::from([(7, 1)]);

    let updates = collect_definition_updates(&lines, &mut mapping);

    assert_eq!(updates.is_definition_line, vec![false, false, true]);
    assert_eq!(
        updates
            .definitions
            .iter()
            .map(|definition| definition.line.as_str())
            .collect::<Vec<_>>(),
        vec!["[^2]: Numeric note"]
    );
}

#[test]
fn rewrite_definition_headers_updates_only_known_definition_lines() {
    let mut lines = strings(&["[^7]: Old", "text"]);
    let definitions = vec![DefinitionLine {
        index: 0,
        new_number: 1,
        line: "[^1]: New".to_string(),
    }];

    rewrite_definition_headers(&mut lines, &definitions);

    assert_eq!(lines, strings(&["[^1]: New", "text"]));
}

#[test]
fn reorder_definition_block_sorts_segments_by_new_number() {
    let mut lines = strings(&[
        "## Footnotes",
        "",
        "[^7]: Second",
        "    continuation",
        "",
        "[^3]: First",
    ]);
    let definitions = vec![
        DefinitionLine {
            index: 2,
            new_number: 2,
            line: "[^2]: Second".to_string(),
        },
        DefinitionLine {
            index: 5,
            new_number: 1,
            line: "[^1]: First".to_string(),
        },
    ];

    reorder_definition_block(&mut lines, 0, 6, &definitions);

    assert_eq!(
        lines,
        strings(&[
            "## Footnotes",
            "",
            "[^1]: First",
            "",
            "[^2]: Second",
            "    continuation",
        ])
    );
}

proptest! {
    #[test]
    fn reorder_definition_block_orders_generated_definition_mappings(
        numbers in proptest::collection::vec(1usize..=20, 2..=8),
    ) {
        let mut lines = vec!["## Footnotes".to_string(), String::new()];
        let mut definitions = Vec::new();
        for (offset, number) in numbers.iter().enumerate() {
            let index = lines.len();
            lines.push(format!("[^{number}]: Body {offset}"));
            definitions.push(DefinitionLine {
                index,
                new_number: *number,
                line: format!("[^{number}]: Body {offset}"),
            });
        }

        reorder_definition_block(&mut lines, 0, definitions.len() + 2, &definitions);

        let emitted_numbers = lines
            .iter()
            .filter_map(|line| line.strip_prefix("[^"))
            .filter_map(|rest| rest.split("]:").next())
            .filter_map(|number| number.parse::<usize>().ok())
            .collect::<Vec<_>>();
        let mut sorted = emitted_numbers.clone();
        sorted.sort_unstable();
        prop_assert_eq!(emitted_numbers, sorted);
    }
}
