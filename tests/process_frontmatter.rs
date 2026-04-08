//! Tests for YAML frontmatter handling in process functions.

use mdtablefix::process::{Options, process_stream, process_stream_inner};
use rstest::rstest;

#[rstest]
#[case(
    vec!["---", "title: Example", "author: Test", "---", "# Heading", "|A|B|", "|1|2|"],
    true,
    Some(vec!["---", "title: Example", "author: Test", "---"]),
)]
#[case(
    vec!["---", "title: Example", "...", "Body text"],
    true,
    Some(vec!["---", "title: Example", "...", "Body text"]),
)]
#[case(
    vec!["# Heading", "|A|B|", "|1|2|"],
    false,
    None,
)]
#[case(
    vec!["---", "Not frontmatter", "More text"],
    false,
    None,
)]
fn frontmatter_detection_behaviour(
    #[case] raw: Vec<&str>,
    #[case] has_frontmatter: bool,
    #[case] expected_prefix: Option<Vec<&str>>,
) {
    let first_line = raw[0].to_string();
    let input: Vec<String> = raw.into_iter().map(str::to_string).collect();
    let out = process_stream(&input);
    assert!(!out.is_empty());

    if has_frontmatter {
        if let Some(prefix) = expected_prefix {
            for (i, expected_line) in prefix.iter().enumerate() {
                assert_eq!(&out[i], *expected_line);
            }
        }
    } else if first_line == "---" {
        // Unmatched opener case: --- is treated as body content
        let joined = out.join("\n");
        assert!(out[0].contains("---"));
        assert!(joined.contains("Not frontmatter"));
        assert!(joined.contains("More text"));
    } else {
        // No frontmatter case: body processed normally
        assert_eq!(out[0], "# Heading");
        assert!(out.len() >= 2);
    }
}

#[test]
fn process_stream_inner_does_not_handle_frontmatter() {
    // process_stream_inner should NOT handle frontmatter - it's the caller's
    // responsibility. This test verifies that behavior.
    let input = vec![
        "---".to_string(),
        "title: Example".to_string(),
        "---".to_string(),
        "# Heading".to_string(),
    ];
    let out = process_stream_inner(
        &input,
        Options {
            headings: false,
            ..Default::default()
        },
    );
    // process_stream_inner doesn't split frontmatter, so --- is treated as body
    // With headings: false, lines should pass through unchanged
    assert_eq!(out[0], "---");
    assert_eq!(out[1], "title: Example");
    assert_eq!(out[2], "---");
    assert_eq!(out[3], "# Heading");
}
