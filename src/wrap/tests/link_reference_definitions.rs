//! Link reference definition wrap tests extracted to keep `tests.rs` below 400 lines.

use rstest::rstest;

use crate::wrap::wrap_text;

#[test]
fn wrap_text_preserves_inline_link_reference_title() {
    let input = vec!["[example]: https://example.com \"Example site\"".to_string()];
    assert_eq!(wrap_text(&input, 80), input);
}

#[rstest]
#[case("  \"Ansible documentation\"")]
#[case("'Ansible documentation'")]
#[case("(Ansible documentation)")]
fn wrap_text_preserves_link_reference_title_on_next_line(#[case] title_line: &str) {
    let input = vec![
        "[ansible]: <https://docs.ansible.com/>".to_string(),
        title_line.to_string(),
    ];
    assert_eq!(wrap_text(&input, 80), input);
}

#[test]
fn wrap_text_reflows_paragraph_after_link_reference_title() {
    let paragraph = concat!(
        "This paragraph is long enough that it should wrap across multiple ",
        "lines when processed with a narrow wrap width."
    );
    let input = vec![
        "[ansible]: <https://docs.ansible.com/>".to_string(),
        "  \"Ansible documentation\"".to_string(),
        paragraph.to_string(),
    ];
    let wrapped = wrap_text(&input, 40);

    assert_eq!(&wrapped[0..2], &input[0..2]);
    assert!(wrapped.len() > 3);
    assert!(wrapped.iter().all(|line| line.len() <= 40));
}

#[test]
fn wrap_text_treats_title_after_blank_line_as_prose() {
    let candidate_title = format!("  \"{}\"", "word ".repeat(20));
    let input = vec![
        "[ansible]: <https://docs.ansible.com/>".to_string(),
        String::new(),
        candidate_title,
    ];
    let wrapped = wrap_text(&input, 40);

    assert_eq!(wrapped[0], input[0]);
    assert_eq!(wrapped[1], "");
    assert!(wrapped.len() > 3);
}

#[test]
fn wrap_text_clears_awaiting_link_title_at_fence_opener() {
    let input = vec![
        "[foo]: https://example.com".to_string(),
        "```python".to_string(),
        "code".to_string(),
        "```".to_string(),
    ];
    assert_eq!(wrap_text(&input, 80), input);
}

#[test]
fn wrap_text_reflows_prose_after_bare_link_reference_definition() {
    let paragraph = concat!(
        "Paragraph text here continues with enough words to require ",
        "reflow when wrapped at a narrow width."
    );
    let input = vec![
        "[foo]: https://example.com".to_string(),
        paragraph.to_string(),
    ];
    let wrapped = wrap_text(&input, 20);

    assert_eq!(wrapped[0], input[0]);
    assert!(wrapped.len() > 2);
    for line in wrapped.iter().skip(1) {
        assert!(line.len() <= 20);
    }
}

#[test]
fn wrap_text_reflows_indented_list_after_label_only_reference() {
    let item = concat!(
        " - a very long list item follows a label-only reference and ",
        "must still be handled by the list wrapping path."
    );
    let input = vec!["[foo]:".to_string(), item.to_string()];
    let wrapped = wrap_text(&input, 36);

    assert_eq!(wrapped[0], input[0]);
    assert!(wrapped.len() > 2);
    for line in wrapped.iter().skip(1) {
        assert!(line.len() <= 36);
    }
}

#[test]
fn wrap_text_does_not_apply_awaiting_link_title_inside_fence() {
    let input = vec![
        "```".to_string(),
        "[foo]: https://example.com".to_string(),
        "\"A title\"".to_string(),
        "```".to_string(),
    ];
    assert_eq!(wrap_text(&input, 80), input);
}
