//! Unit tests for footnote renumbering helpers.
//!
//! These cases exercise the private numeric-candidate parsing and renumbering
//! helpers directly so the behaviour stays covered without bloating the main
//! module file.

use rstest::rstest;

use super::{numeric_candidate_from_line, renumber_footnotes};

fn strings(lines: &[&str]) -> Vec<String> { lines.iter().map(|line| (*line).to_string()).collect() }

#[rstest]
#[case("7.")]
#[case("7:")]
fn malformed_numeric_candidate_line_is_ignored(#[case] line: &str) {
    assert!(
        numeric_candidate_from_line(line, 0).is_none(),
        "offending line: {line:?}"
    );
}

#[rstest]
#[case::existing_definition(
    strings(&["Reference.[^7]", "", "[^7]: Existing definition"]),
    strings(&["Reference.[^1]", "", "[^1]: Existing definition"]),
)]
#[case::numeric_candidate(
    strings(&["Reference.[^7]", "", "7. Legacy footnote"]),
    strings(&["Reference.[^1]", "", "[^1]: Legacy footnote"]),
)]
fn renumber_footnotes_rewrites_definitions(
    #[case] mut input: Vec<String>,
    #[case] expected: Vec<String>,
) {
    renumber_footnotes(&mut input);
    assert_eq!(input, expected);
}

mod proptest_tests {
    //! Property tests for footnote renumbering.
    //!
    //! These cases use `proptest` and `Regex` to generate reference and
    //! definition sets, then verify that renumbering preserves fenced content
    //! while assigning sequential footnote numbers.

    use proptest::prelude::*;
    use regex::Regex;

    use super::renumber_footnotes;

    /// Distinct numbers in first-appearance order.
    fn unique_in_order(numbers: &[usize]) -> Vec<usize> {
        let mut seen = Vec::new();
        for &n in numbers {
            if !seen.contains(&n) {
                seen.push(n);
            }
        }
        seen
    }

    proptest! {
        #[test]
        fn renumber_footnotes_assigns_sequential_numbers_and_preserves_fenced_refs(
            numbers in proptest::collection::vec(1usize..=20, 1..=10),
        ) {
            let unique = unique_in_order(&numbers);

            // Layout: refs, blank, fenced block (unchanged ref), blank, definitions.
            let mut input: Vec<String> = Vec::new();
            for &n in &numbers {
                input.push(format!("Reference[^{n}]."));
            }
            input.push(String::new());
            // Use a footnote number that cannot collide with any generated value
            // (we only generate 1..=20) so any rewrite of this line would be
            // obviously wrong.
            let fenced_reference = "inside [^999] fence".to_string();
            input.push("```".to_string());
            input.push(fenced_reference.clone());
            input.push("```".to_string());
            input.push(String::new());
            let definitions_start = input.len();
            for &n in &unique {
                input.push(format!("[^{n}]: Body for {n}"));
            }

            renumber_footnotes(&mut input);

            // 1. References in non-fenced text map to their definition's new number.
            let mapping: std::collections::HashMap<usize, usize> = unique
                .iter()
                .enumerate()
                .map(|(i, &n)| (n, i + 1))
                .collect();
            for (i, &n) in numbers.iter().enumerate() {
                let new_number = mapping[&n];
                prop_assert_eq!(input[i].clone(), format!("Reference[^{new_number}]."));
            }

            // 2. Fenced block is untouched, including its embedded `[^999]` ref.
            let fence_idx = numbers.len() + 1;
            prop_assert_eq!(input[fence_idx].clone(), "```".to_string());
            prop_assert_eq!(input[fence_idx + 1].clone(), fenced_reference);
            prop_assert_eq!(input[fence_idx + 2].clone(), "```".to_string());

            // 3. Definitions are consecutively numbered 1..=unique.len() with no gaps.
            let header_re = Regex::new(r"^\[\^(\d+)\]:").expect("header regex compiles");
            let mut seen_numbers: Vec<usize> = input[definitions_start..]
                .iter()
                .filter_map(|line| {
                    header_re.captures(line).and_then(|c| c.get(1)?.as_str().parse().ok())
                })
                .collect();
            seen_numbers.sort_unstable();
            let expected_numbers: Vec<usize> = (1..=unique.len()).collect();
            prop_assert_eq!(seen_numbers, expected_numbers);
        }
    }
}
