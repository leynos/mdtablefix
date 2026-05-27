//! Link reference definition wrapping tests.
//!
//! Validates that CommonMark link reference definitions remain verbatim during
//! wrapping and are not collapsed into prose paragraphs.

use rstest::rstest;

use super::*;

#[rstest]
#[case(lines_vec!["[ansible]: <https://docs.ansible.com/projects/ansible/latest/>"])]
#[case(lines_vec![
    "[ansible]: <https://docs.ansible.com/projects/ansible/latest/playbook_guide/playbooks_conditionals.html>",
    "[bazel]: <https://bazel.build/docs/configurable-attributes>",
    "[cargo-make]: <https://sagiegurari.github.io/cargo-make/>",
    "[github-actions]: <https://docs.github.com/actions/writing-workflows/choosing-what-your-workflow-does/running-variations-of-jobs-in-a-workflow>",
    "[gnu-make]: <https://web.mit.edu/gnu/doc/html/make_7.html>",
    "[just]: <https://just.systems/man/en/conditional-expressions.html>",
    "[taskfile]: <https://taskfile.dev/docs/guide>",
])]
#[case(lines_vec!["[example]: https://example.com/path"])]
#[case(lines_vec!["[example]: https://example.com \"Example site\""])]
fn test_wrap_link_reference_definitions_passthrough(input: Vec<String>) {
    let output = process_stream(&input);
    assert_eq!(output, input);
}

#[test]
fn test_wrap_link_reference_definitions_mixed_with_paragraph() {
    let input = lines_vec![
        "[ansible]: <https://docs.ansible.com/>",
        concat!(
            "This paragraph is long enough that it should wrap across multiple ",
            "lines when processed by the wrapping pass."
        ),
        "[bazel]: <https://bazel.build/>",
    ];
    let output = process_stream(&input);

    assert_eq!(output[0], input[0]);
    assert_eq!(output.last().expect("output ends with link reference"), input[2]);
    assert!(output.len() > 3);
    assert!(output.iter().all(|line| line.len() <= 80));
}

#[test]
fn test_wrap_link_reference_definition_at_document_start() {
    let input = lines_vec![
        "[first]: <https://example.com/first>",
        "Short paragraph.",
    ];
    let output = process_stream(&input);
    assert_eq!(output[0], input[0]);
}

#[test]
fn test_wrap_link_reference_definition_at_document_end() {
    let input = lines_vec![
        "Short paragraph.",
        "[last]: <https://example.com/last>",
    ];
    let output = process_stream(&input);
    assert_eq!(output.last().expect("output ends with link reference"), input[1]);
}
