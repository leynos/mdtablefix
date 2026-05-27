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
