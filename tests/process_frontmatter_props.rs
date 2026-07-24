//! Property tests for the canonical frontmatter processing boundary.

use mdtablefix::process::process_with_frontmatter;
use proptest::prelude::*;

fn copy_body(body: &[String]) -> Vec<String> { body.to_vec() }

prop_compose! {
    fn frontmatter_and_body_strategy()(
        yaml_lines in prop::collection::vec("[a-z]{1,8}: [a-z0-9 ]{0,16}", 0..=6),
        closer in prop_oneof![Just("---"), Just("...")],
        body_lines in prop::collection::vec(
            prop_oneof![
                "[A-Za-z][A-Za-z0-9 ,.]{0,40}",
                "\\|[A-Za-z]{0,8}\\|[A-Za-z]{0,8}\\|",
                "[1-9][0-9]?\\. [A-Za-z][A-Za-z0-9 ]{0,30}",
            ],
            0..=12,
        ),
    ) -> (Vec<String>, Vec<String>) {
        let mut prefix = vec!["---".to_string()];
        prefix.extend(yaml_lines);
        prefix.push(closer.to_string());
        (prefix, body_lines)
    }
}

prop_compose! {
    fn body_strategy()(
        body_lines in prop::collection::vec(
            prop_oneof![
                "[A-Za-z][A-Za-z0-9 ,.]{0,40}",
                "\\|[A-Za-z]{0,8}\\|[A-Za-z]{0,8}\\|",
                "[1-9][0-9]?\\. [A-Za-z][A-Za-z0-9 ]{0,30}",
            ],
            0..=12,
        ),
    ) -> Vec<String> {
        body_lines
    }
}

proptest! {
    #[test]
    fn preserves_frontmatter_byte_identically((prefix, body) in frontmatter_and_body_strategy()) {
        let mut input = prefix.clone();
        input.extend(body);

        let output = process_with_frontmatter(&input, copy_body);

        prop_assert!(output.starts_with(&prefix));
    }

    #[test]
    fn passes_only_the_post_frontmatter_body_to_the_closure(
        (prefix, body) in frontmatter_and_body_strategy(),
    ) {
        let mut input = prefix.clone();
        input.extend(body.clone());
        let mut received = Vec::new();

        let _output = process_with_frontmatter(&input, |body| {
            received = body.to_vec();
            body.to_vec()
        });

        prop_assert_eq!(&received, &body);
        prop_assert!(!received.iter().any(|line| line == "---" || line == "..."));
    }

    #[test]
    fn passes_the_whole_input_to_the_closure_when_frontmatter_is_absent(body in body_strategy()) {
        let mut received = Vec::new();
        let expected: Vec<String> = body.iter().map(|line| format!("processed: {line}")).collect();

        let output = process_with_frontmatter(&body, |input| {
            received = input.to_vec();
            expected.clone()
        });

        prop_assert_eq!(received, body);
        prop_assert_eq!(output, expected);
    }
}
