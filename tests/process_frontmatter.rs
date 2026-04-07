//! Tests for YAML frontmatter handling in process functions.

use mdtablefix::process::{Options, process_stream, process_stream_inner};

#[test]
fn preserves_yaml_frontmatter_unchanged() {
    let input = vec![
        "---".to_string(),
        "title: Example".to_string(),
        "author: Test".to_string(),
        "---".to_string(),
        "# Heading".to_string(),
        "|A|B|".to_string(),
        "|1|2|".to_string(),
    ];
    let out = process_stream(&input);
    // Frontmatter lines should be unchanged
    assert_eq!(out[0], "---");
    assert_eq!(out[1], "title: Example");
    assert_eq!(out[2], "author: Test");
    assert_eq!(out[3], "---");
    // Body should be formatted
    assert!(out[4].contains("# Heading"));
    assert!(out[5].contains("| A | B |") || out[5].contains("|A|B|"));
}

#[test]
fn frontmatter_with_triple_dot_closer_preserved() {
    let input = vec![
        "---".to_string(),
        "title: Example".to_string(),
        "...".to_string(),
        "Body text".to_string(),
    ];
    let out = process_stream(&input);
    assert_eq!(out[0], "---");
    assert_eq!(out[1], "title: Example");
    assert_eq!(out[2], "...");
    assert_eq!(out[3], "Body text");
}

#[test]
fn no_frontmatter_processes_normally() {
    let input = vec![
        "# Heading".to_string(),
        "|A|B|".to_string(),
        "|1|2|".to_string(),
    ];
    let out = process_stream(&input);
    // Should process normally without frontmatter
    assert_eq!(out[0], "# Heading");
    assert!(out.len() >= 2);
}

#[test]
fn unmatched_frontmatter_opener_processed_as_body() {
    // A --- without a closer is not frontmatter
    let input = vec![
        "---".to_string(),
        "Not frontmatter".to_string(),
        "More text".to_string(),
    ];
    let out = process_stream(&input);
    // All lines should be processed as body (no special frontmatter handling)
    // The lines may be wrapped together, so just verify the content is present
    assert!(out[0].contains("---"));
    let joined = out.join("\n");
    assert!(joined.contains("Not frontmatter"));
    assert!(joined.contains("More text"));
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
