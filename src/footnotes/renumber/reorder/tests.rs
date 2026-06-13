//! Direct coverage for the definition-block reordering helpers in
//! [`super`](super).

use proptest::prelude::*;

use super::{DefinitionLine, reorder_definition_block};

fn strings(lines: &[&str]) -> Vec<String> { lines.iter().map(|line| (*line).to_string()).collect() }

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

        let original_len = lines.len();
        reorder_definition_block(&mut lines, 0, definitions.len() + 2, &definitions);

        let emitted_numbers = lines
            .iter()
            .filter_map(|line| line.strip_prefix("[^"))
            .filter_map(|rest| rest.split("]:").next())
            .filter_map(|number| number.parse::<usize>().ok())
            .collect::<Vec<_>>();
        let mut sorted = emitted_numbers.clone();
        sorted.sort_unstable();
        prop_assert_eq!(
            lines.len(),
            original_len,
            "reorder_definition_block must preserve line count"
        );
        prop_assert_eq!(emitted_numbers, sorted);
    }
}
